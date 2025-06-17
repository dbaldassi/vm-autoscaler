
use virt::connect::Connect;
use virt::domain::Domain;
use virt_sys::VIR_DOMAIN_INTERFACE_ADDRESSES_SRC_AGENT;
use virt_sys::VIR_IP_ADDR_TYPE_IPV4;

// use virt::InterfaceAddressesSource;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use std::time::Instant;

use sysinfo::{System};

const ROOT_XML_PATH: &str = "/home/tobias/Documents/medooze-vm-monitor/root.xml";
const ROOT_IMG_PATH: &str = "/var/lib/libvirt/images/medooze.qcow2";
const VM_XML_PATH: &str = "/tmp/vm-xml";
const VM_IMG_PATH: &str = "/tmp/vm-img";
const VM_INITIAL_MEMORY: u64 = 4 * 1024 * 1024 * 1024; // 4 GB
const VM_SAFETY_MEMORY: u64 = 2 * 1024 * 1024 * 1024; // 1 GB

struct XmlConfig {
    vm_name: String,
    disk_img: String, // in MB
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

fn main() {
    let mut sys = System::new_all();
    let mut domains = Vec::new();
    let mut conn = Connect::open(Some("qemu:///system")).expect("Failed to connect to hypervisor");
    // let domain = Domain::lookup_by_name(&mut conn, "medooze").expect("Failed to find domain");

    // domain.create().expect("Failed to start domain");

    // println!("Pwd : {}", std::env::current_dir().unwrap().display());

    let mut num_vm = 0; 

    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();

    ctrlc::set_handler(move || {
        println!("Exiting...");
        r.store(false, Ordering::SeqCst);
    }).expect("Error setting Ctrl-C handler");

    let initial_free_memory = fetch_free_memory(&mut sys);
    let start = Instant::now();

    std::fs::create_dir_all(VM_IMG_PATH).expect(format!("Failed to create {} directory", VM_IMG_PATH).as_str());

    while running.load(Ordering::SeqCst) {
        let free_memory = fetch_free_memory(&mut sys);

        if free_memory > VM_INITIAL_MEMORY + VM_SAFETY_MEMORY {
            let config = XmlConfig {
                vm_name: format!("sfu-{}", num_vm),
                disk_img: format!("{}/sfu-{}.qcow2", VM_IMG_PATH, num_vm), // 1 GB
            };

            println!("Creating VM {} with disk image", config.vm_name);

            let xml = create_xml_from_template(&config);
            create_disk_image(&config.disk_img); 
            let domain = define_new_vm(&mut conn, &xml);
            domain.create().expect("Failed to start new VM");
            num_vm += 1;
            
            std::thread::sleep(std::time::Duration::from_secs(60));
            
            let ipv4 = get_vm_ipv4(&domain);
            domains.push(domain);
            println!("VM {} created with IPv4: {}", config.vm_name, ipv4);

        } else {
            std::thread::sleep(std::time::Duration::from_secs(1));
        }
    }

    let elapsed = start.elapsed();

    println!("Created {} VMs of {} GB initial memory in {:.2?} secondes", num_vm, VM_INITIAL_MEMORY / (1024 
        * 1024 * 1024), elapsed);
    println!("Free memory at start: {} GB", initial_free_memory / (1024 * 1024 * 1024));

    // cleanup
    for domain in domains {
        domain.destroy().expect("Failed to shutdown domain");
        domain.undefine().expect("Failed to undefine domain");
    }

    conn.close().expect("Failed to close connection");
}
