#!/usr/bin/env python3

import sys
import polars as pl
import matplotlib.pyplot as plt
import os

if len(sys.argv) < 2:
    print("Usage: python plot_power_vm.py <csv_file>")
    sys.exit(1)

csv_file = sys.argv[1]
df = pl.read_csv(csv_file)

timestamp = df["timestamp"].to_numpy()
outdir = os.path.dirname(os.path.abspath(csv_file))
basename = os.path.splitext(os.path.basename(csv_file))[0]

fig, axes = plt.subplots(3, 2, figsize=(14, 12))
fig.suptitle("Mesures système et nombre de VM en fonction du temps")

# 1. Watts
ax1 = axes[0, 0]
ax1.plot(timestamp, df["watts"].to_numpy(), color='tab:blue')
ax1.set_ylabel("Watts", color='tab:blue')
ax1.tick_params(axis='y', labelcolor='tab:blue')
ax1.set_xlabel("Timestamp")
ax1b = ax1.twinx()
ax1b.plot(timestamp, df["num_vm"].to_numpy(), color='tab:red')
ax1b.set_ylabel("Nombre de VM", color='tab:red')
ax1b.tick_params(axis='y', labelcolor='tab:red')
ax1.set_title("Watts et nombre de VM")

# 2. Mémoire
ax2 = axes[0, 1]
ax2.plot(timestamp, df["memory_usage"].to_numpy(), color='tab:green')
ax2.set_ylabel("Mémoire (MB)", color='tab:green')
ax2.tick_params(axis='y', labelcolor='tab:green')
ax2.set_xlabel("Timestamp")
ax2b = ax2.twinx()
ax2b.plot(timestamp, df["num_vm"].to_numpy(), color='tab:red')
ax2b.set_ylabel("Nombre de VM", color='tab:red')
ax2b.tick_params(axis='y', labelcolor='tab:red')
ax2.set_title("Mémoire et nombre de VM")

# 3. CPU
ax3 = axes[1, 0]
ax3.plot(timestamp, df["cpu_usage"].to_numpy(), color='tab:orange')
ax3.set_ylabel("CPU (%)", color='tab:orange')
ax3.tick_params(axis='y', labelcolor='tab:orange')
ax3.set_xlabel("Timestamp")
ax3b = ax3.twinx()
ax3b.plot(timestamp, df["num_vm"].to_numpy(), color='tab:red')
ax3b.set_ylabel("Nombre de VM", color='tab:red')
ax3b.tick_params(axis='y', labelcolor='tab:red')
ax3.set_title("CPU et nombre de VM")

# 4. Volts
ax4 = axes[1, 1]
ax4.plot(timestamp, df["volts"].to_numpy(), color='tab:purple')
ax4.set_ylabel("Volts", color='tab:purple')
ax4.tick_params(axis='y', labelcolor='tab:purple')
ax4.set_xlabel("Timestamp")
ax4b = ax4.twinx()
ax4b.plot(timestamp, df["num_vm"].to_numpy(), color='tab:red')
ax4b.set_ylabel("Nombre de VM", color='tab:red')
ax4b.tick_params(axis='y', labelcolor='tab:red')
ax4.set_title("Volts et nombre de VM")

# 5. Ampères
ax5 = axes[2, 0]
ax5.plot(timestamp, df["amps"].to_numpy(), color='tab:brown')
ax5.set_ylabel("Ampères", color='tab:brown')
ax5.tick_params(axis='y', labelcolor='tab:brown')
ax5.set_xlabel("Timestamp")
ax5b = ax5.twinx()
ax5b.plot(timestamp, df["num_vm"].to_numpy(), color='tab:red')
ax5b.set_ylabel("Nombre de VM", color='tab:red')
ax5b.tick_params(axis='y', labelcolor='tab:red')
ax5.set_title("Ampères et nombre de VM")

# 6. kWh
ax6 = axes[2, 1]
ax6.plot(timestamp, df["kwh"].to_numpy(), color='tab:pink')
ax6.set_ylabel("kWh", color='tab:pink')
ax6.tick_params(axis='y', labelcolor='tab:pink')
ax6.set_xlabel("Timestamp")
ax6b = ax6.twinx()
ax6b.plot(timestamp, df["num_vm"].to_numpy(), color='tab:red')
ax6b.set_ylabel("Nombre de VM", color='tab:red')
ax6b.tick_params(axis='y', labelcolor='tab:red')
ax6.set_title("kWh et nombre de VM")

plt.tight_layout(rect=[0, 0.03, 1, 0.97])
img_file = os.path.join(outdir, f"{basename}_all_metrics.png")
plt.savefig(img_file)