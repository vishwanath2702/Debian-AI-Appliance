#!/bin/bash

source installer/files/opt/daia/lib/hardware.sh

echo "CPU Model : $(get_cpu_model)"

echo "Architecture : $(get_cpu_arch)"

echo "Cores : $(get_cpu_cores)"

echo "RAM : $(get_total_ram_mb) MB"

echo "Disk : $(get_root_disk)"

echo "Disk Size : $(get_root_disk_size)"

echo "Disk Type : $(get_root_disk_type)"

echo "GPU : $(get_gpu_vendor)"
