
use virt::connect::Connect;
use virt::domain::Domain;
use virt_sys::VIR_DOMAIN_INTERFACE_ADDRESSES_SRC_AGENT;
use virt_sys::VIR_IP_ADDR_TYPE_IPV4;

// use virt::InterfaceAddressesSource;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use std::time::Instant;

use sysinfo::{System};

use std::ffi::CString;

use csv::Writer;
// use serde::Serialize;

// use cgroups_proactive_reclaim;
use cgroups_proactive_reclaim::CgroupsReclaimManager;

const ROOT_XML_PATH: &str = "/home/tobias/Documents/medooze-vm-monitor/root.xml";
const ROOT_IMG_PATH: &str = "/var/lib/libvirt/images/medooze.qcow2";
const VM_XML_PATH: &str = "/tmp/vm-xml";
const VM_IMG_PATH: &str = "/tmp/vm-img";
const VM_INITIAL_MEMORY: u64 = 4 * 1024 * 1024 * 1024; // 4 GB
const VM_SAFETY_MEMORY: u64 = 2 * 1024 * 1024 * 1024; // 1 GB

#[link(name = "wattsup")]
unsafe extern "C" {
    fn wu_get_data(fd: i32, arr: *mut f64) -> i32;
    // fn wu_get_num_metric() -> i32;
    //int open_device(char * device_name, int * dev_fd)
    fn open_device(device_name: *const i8, dev_fd: *mut i32) -> i32;
    // int setup_serial_device(int dev_fd)
    fn setup_serial_device(dev_fd: i32) -> i32;
    fn wu_close(dev_fd: i32);
    fn wu_clear(dev_fd: i32) -> i32;
}

struct XmlConfig {
    vm_name: String,
    disk_img: String, // in MB
}

enum RegulationMethod {
    CgroupsProactiveReclaim(CgroupsReclaimManager),
    // Other methods can be added here
}

struct DomMemStats {
    swap_in: u64,
    swap_out: u64,
    major_fault: u64,
    minor_fault: u64,
    unused: u64,
    available: u64,
    actual_balloon: u64,
    rss: u64,
    usable: u64,
    last_update: u64,
    disk_caches: u64,
    huge_tlb_pgalloc: u64,
    huge_tlb_pgfail: u64,
}

#[derive(serde::Serialize, Default)]
struct LogEntry {
    timestamp: u64,
    cpu_usage: f64,
    memory_usage: u64, // in KB
    num_vm: u32,
    watts: f64,
    volts: f64,
    amps: f64,
    kwh: f64,
    cost: f64,
    mo_kwh: f64,
    mo_cost: f64,
    max_watts: f64,
    min_volts: f64,
    max_amps: f64,
    min_watts: f64,
    max_volts: f64,
    min_amps: f64,
    power_factor: f64,
    duty_cycle: f64,
    power_cycle: f64,
    frequency: f64,
    va: f64,
}

#[repr(u8)]
enum WattsupMetric {
    Watts = 0,
    Volts,
    Amps,
    Kwh,
    Cost,
    MoKwh,
    MoCost,
    MaxWatts,
    MinVolts,
    MaxAmps,
    MinWatts,
    MaxVolts,
    MinAmps,
    PowerFactor,
    DutyCycle,
    PowerCycle,
    Frequency,
    Va,
}

const WATTSUP_NUM_METRICS: usize = 18;

fn create_csv_writer(path: &str) -> Writer<std::fs::File> {
    let file = std::fs::File::create(path).expect("Failed to create CSV file");
    let wtr = Writer::from_writer(file);

    wtr
}

fn write_log_entry(wtr: &mut Writer<std::fs::File>, logentry: &LogEntry) {
    wtr.serialize(logentry).expect("Failed to write log entry to CSV");
    wtr.flush().expect("Failed to flush CSV writer");
}

fn add_wattsup_metrics(logentry: &mut LogEntry, wattsup_data: &[f64; WATTSUP_NUM_METRICS]) {
    logentry.watts = wattsup_data[WattsupMetric::Watts as usize];
    logentry.volts = wattsup_data[WattsupMetric::Volts as usize];
    logentry.amps = wattsup_data[WattsupMetric::Amps as usize];
    logentry.kwh = wattsup_data[WattsupMetric::Kwh as usize];
    logentry.cost = wattsup_data[WattsupMetric::Cost as usize];
    logentry.mo_kwh = wattsup_data[WattsupMetric::MoKwh as usize];
    logentry.mo_cost = wattsup_data[WattsupMetric::MoCost as usize];
    logentry.max_watts = wattsup_data[WattsupMetric::MaxWatts as usize];
    logentry.min_volts = wattsup_data[WattsupMetric::MinVolts as usize];
    logentry.max_amps = wattsup_data[WattsupMetric::MaxAmps as usize];
    logentry.min_watts = wattsup_data[WattsupMetric::MinWatts as usize];
    logentry.max_volts = wattsup_data[WattsupMetric::MaxVolts as usize];
    logentry.min_amps = wattsup_data[WattsupMetric::MinAmps as usize];
    logentry.power_factor = wattsup_data[WattsupMetric::PowerFactor as usize];
    logentry.duty_cycle = wattsup_data[WattsupMetric::DutyCycle as usize];
    logentry.power_cycle = wattsup_data[WattsupMetric::PowerCycle as usize];
    logentry.frequency = wattsup_data[WattsupMetric::Frequency as usize];
    logentry.va = wattsup_data[WattsupMetric::Va as usize].into();
}

fn create_xml_from_template(config: &XmlConfig) -> String {
    let xml = std::fs::read_to_string(ROOT_XML_PATH).expect("Failed to read root XML file");
    let mut xml = xml.replace("__VM_NAME__", &config.vm_name);
    xml = xml.replace("__DISKIMG__", &config.disk_img);
    
    // create xml path if it does not exist
    std::fs::create_dir_all(VM_XML_PATH).expect(format!("Failed to create {} directory", VM_XML_PATH).as_str());
    // concatenate the path with the VM name
    let vm_xml_path = format!("{}/{}.xml", VM_XML_PATH, config.vm_name);
    // write the modified XML to a new file
    std::fs::write(&vm_xml_path, xml).expect("Failed to write VM XML file");

    vm_xml_path
}

fn create_disk_image(img_dest: &str) -> () {

    //  qemu-img create -f qcow2 -b "/var/lib/libvirt/images/medooze.qcow2" -F qcow2 sfu4.qcow2

    // create the disk image with qemu-img
    let output = std::process::Command::new("/usr/bin/qemu-img")
        .arg("create")
        .arg("-f")
        .arg("qcow2")
        .arg("-b")
        .arg(ROOT_IMG_PATH) // base image
        .arg("-F")
        .arg("qcow2")
        .arg(img_dest)
        .output()
        .expect("Failed to create disk image");
    
    if !output.status.success() {
        panic!("Failed to create disk image: {}", String::from_utf8_lossy(&output.stderr));
    }
}

fn fetch_free_memory(sys: &mut System) -> u64 {
    sys.refresh_all();
    let free_memory = sys.free_memory(); // in KB
    free_memory
}

fn define_new_vm(conn: &Connect, xml_path: &str) -> Domain {
    let xml = std::fs::read_to_string(xml_path).expect("Failed to read VM XML file");
    let domain = Domain::define_xml(conn, &xml).expect("Failed to define new VM");
    domain
}

fn get_vm_ipv4(domain: &Domain) -> String {
    let interfaces = domain.interface_addresses(VIR_DOMAIN_INTERFACE_ADDRESSES_SRC_AGENT, 0).expect("Failed to get interface addresses");
    
    for interface in interfaces {
        // println!("Interface: {}", interface.name);
        if interface.name == "enp8s0" { // assuming eth0 is the interface we want
            let addr = interface.addrs.iter()
                .find(|addr| addr.typed == VIR_IP_ADDR_TYPE_IPV4.into());
            if let Some(addr) = addr {
                // Return the first IPv4 address found
                return addr.addr.clone();
            }
            
        }
    }
    
    String::new() // return empty string if no IPv4 address found
}

fn init_wattsup() -> i32 {
    let mut dev_fd: i32 = 0;
    let device_name = CString::new("/dev/ttyUSB0").expect("CString::new failed");
    
    unsafe {
        if open_device(device_name.as_ptr(), &mut dev_fd) != 0 {
            println!("Failed to open WattsUp device");
            return -1;
        }
        if setup_serial_device(dev_fd) != 0 {
            println!("Failed to setup WattsUp device");
            wu_close(dev_fd);
            return -1;
        }

        wu_clear(dev_fd);
    }
    
    dev_fd
}

fn fetch_dommemstats(domain: &Domain) -> DomMemStats {
    let stats = domain.memory_stats(0).expect("Failed to get memory stats");
    
    let mut dom_mem_stats = DomMemStats {
        swap_in: 0,
        swap_out: 0,
        major_fault: 0,
        minor_fault: 0,
        unused: 0,
        available: 0,
        actual_balloon: 0,
        rss: 0,
        usable: 0,
        last_update: 0,
        disk_caches: 0,
        huge_tlb_pgalloc: 0,
        huge_tlb_pgfail: 0,
    };

    for s in stats {
        match s.tag {
            virt_sys::VIR_DOMAIN_MEMORY_STAT_SWAP_IN => dom_mem_stats.swap_in = s.val,
            virt_sys::VIR_DOMAIN_MEMORY_STAT_SWAP_OUT => dom_mem_stats.swap_out = s.val,
            virt_sys::VIR_DOMAIN_MEMORY_STAT_MAJOR_FAULT => dom_mem_stats.major_fault = s.val,
            virt_sys::VIR_DOMAIN_MEMORY_STAT_MINOR_FAULT => dom_mem_stats.minor_fault = s.val,
            virt_sys::VIR_DOMAIN_MEMORY_STAT_UNUSED => dom_mem_stats.unused = s.val,
            virt_sys::VIR_DOMAIN_MEMORY_STAT_AVAILABLE => dom_mem_stats.available = s.val,
            virt_sys::VIR_DOMAIN_MEMORY_STAT_ACTUAL_BALLOON => dom_mem_stats.actual_balloon = s.val,
            virt_sys::VIR_DOMAIN_MEMORY_STAT_RSS => dom_mem_stats.rss = s.val,
            virt_sys::VIR_DOMAIN_MEMORY_STAT_USABLE => dom_mem_stats.usable = s.val,
            virt_sys::VIR_DOMAIN_MEMORY_STAT_LAST_UPDATE => dom_mem_stats.last_update = s.val,
            virt_sys::VIR_DOMAIN_MEMORY_STAT_DISK_CACHES => dom_mem_stats.disk_caches = s.val,
            virt_sys::VIR_DOMAIN_MEMORY_STAT_HUGETLB_PGALLOC => dom_mem_stats.huge_tlb_pgalloc = s.val,
            virt_sys::VIR_DOMAIN_MEMORY_STAT_HUGETLB_PGFAIL => dom_mem_stats.huge_tlb_pgfail = s.val,
            _ => {}
        }
    }

    dom_mem_stats
}

fn regulate_domain(domain: &Domain, method: &mut RegulationMethod) {
    const THRESHOLD : u64 = 1024 * 1024 * 200; // 200 MB

    let stats = fetch_dommemstats(domain);

    let swap = stats.swap_out - stats.swap_in;
    let used = stats.available - stats.usable + swap;

    println!("Swap In: {}, Swap Out: {}, Available: {}, Used Memory: {}", stats.swap_in, stats.swap_out, stats.available, used);

    match method {
        RegulationMethod::CgroupsProactiveReclaim(manager) => {
            // Call the proactive reclaim method
            manager.regulate(used, THRESHOLD).expect("Failed to regulate domain with CgroupsProactiveReclaim");
        }
        // Add other methods here
    }
}

fn main() {
    let mut sys = System::new_all();
    let mut domains: Vec<(Domain, RegulationMethod)> = Vec::new();
    let mut conn = Connect::open(Some("qemu:///system")).expect("Failed to connect to hypervisor");

    let wattsup = init_wattsup();
    if wattsup < 0 {
        std::process::exit(1);
    }

    println!("WattsUp device initialized successfully");
    
    let mut wattsup_data:[f64; WATTSUP_NUM_METRICS] = [0.; WATTSUP_NUM_METRICS];

    let mut num_vm = 0; 

    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();

    ctrlc::set_handler(move || {
        println!("Exiting...");
        r.store(false, Ordering::SeqCst);
    }).expect("Error setting Ctrl-C handler");

    let initial_free_memory = fetch_free_memory(&mut sys);
    let start = Instant::now();
    let mut time : u64 = 0;

    const WAIT_TIME : u64 = 10; // seconds
    let mut time_since_vm_creation = 0;

    std::fs::create_dir_all(VM_IMG_PATH).expect(format!("Failed to create {} directory", VM_IMG_PATH).as_str());

    let mut wtr = create_csv_writer("vm_autoscaler_log.csv");

    while running.load(Ordering::SeqCst) {
        let free_memory = fetch_free_memory(&mut sys);

        for (domain, regulation) in &mut domains {
            regulate_domain(&domain, regulation);
        }

        if time_since_vm_creation >= WAIT_TIME && free_memory > VM_INITIAL_MEMORY + VM_SAFETY_MEMORY {
            let config = XmlConfig {
                vm_name: format!("sfu{}", num_vm),
                disk_img: format!("{}/sfu{}.qcow2", VM_IMG_PATH, num_vm), // 1 GB
            };

            println!("Creating VM {} with disk image", config.vm_name);

            let xml = create_xml_from_template(&config);
            create_disk_image(&config.disk_img); 
            let domain = define_new_vm(&mut conn, &xml);
            domain.create().expect("Failed to start new VM");
            num_vm += 1;
            // let ipv4 = get_vm_ipv4(&domain);
            
            std::thread::sleep(std::time::Duration::from_secs(1)); // wait for the VM to start
            time += 1;

            let cgroup_path = cgroups_proactive_reclaim::get_cgroup_path(&config.vm_name)
            .expect("Failed to get cgroup path");
            let cgroups_manager = CgroupsReclaimManager::new(&cgroup_path);
            
            domains.push((domain, RegulationMethod::CgroupsProactiveReclaim(cgroups_manager)));

            time_since_vm_creation = 0;
        } else {
            time_since_vm_creation += 1;
        }

        unsafe {
            wu_get_data(wattsup, wattsup_data.as_mut_ptr());
        }

        let mut logentry = LogEntry {
            timestamp: time,
            cpu_usage: sys.global_cpu_usage() as f64,
            memory_usage: sys.used_memory() / (1024 * 1024), // convert to MB
            num_vm: num_vm as u32,
            ..LogEntry::default()
        };

        add_wattsup_metrics(&mut logentry, &wattsup_data);

        write_log_entry(&mut wtr, &logentry);

        std::thread::sleep(std::time::Duration::from_secs(1));
        time += 1;
    }

    unsafe {
        wu_close(wattsup);
    }

    let elapsed = start.elapsed();

    println!("Created {} VMs of {} GB initial memory in {:.2?} secondes", num_vm, VM_INITIAL_MEMORY / (1024 
        * 1024 * 1024), elapsed);
    println!("Free memory at start: {} GB", initial_free_memory / (1024 * 1024 * 1024));

    // cleanup
    for (domain,_) in &domains {
        domain.destroy().expect("Failed to shutdown domain");
        domain.undefine().expect("Failed to undefine domain");
    }

    conn.close().expect("Failed to close connection");
}
