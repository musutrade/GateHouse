#!/usr/bin/env bash
# Background resource sampler for CI monitoring

set -euo pipefail

MONITOR_DIR="${CI_MONITOR_DIR:-target/ci-monitor}"
INTERVAL="${CI_MONITOR_INTERVAL:-5}"
MAX_DURATION="${CI_MONITOR_MAX_DURATION:-3600}"
PID_FILE="${MONITOR_DIR}/sampler.pid"

mkdir -p "$MONITOR_DIR"

# Record our PID for cleanup
echo $$ > "$PID_FILE"

# Trap signals for clean exit
trap 'rm -f "$PID_FILE"; exit 0' SIGTERM SIGINT

start_time=$(date +%s)
sample_count=0

# Previous CPU stats for delta calculation
prev_cpu_total=0
prev_cpu_idle=0
prev_cpu_iowait=0
prev_cpu_steal=0

# Previous disk stats for delta calculation
prev_disk_read_sectors=0
prev_disk_write_sectors=0
prev_sample_time=0

echo "[CI Monitor] Starting resource sampler (interval=${INTERVAL}s, max_duration=${MAX_DURATION}s)"

while true; do
    current_time=$(date +%s)
    elapsed=$((current_time - start_time))
    
    if [ $elapsed -ge $MAX_DURATION ]; then
        echo "[CI Monitor] Max duration reached, stopping sampler"
        break
    fi
    
    # Collect system snapshot
    timestamp=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
    output_file="${MONITOR_DIR}/sample_$(date +%s)_$$.json"
    
    # CPU info
    cpu_count=$(nproc 2>/dev/null || echo "unknown")
    
    # CPU usage from /proc/stat (interval-based)
    cpu_usage_percent="null"
    cpu_iowait_percent="null"
    cpu_steal_percent="null"
    disk_device="null"
    if [ -f /proc/stat ] && [ $sample_count -gt 0 ]; then
        # Read first line: cpu user nice system idle iowait irq softirq steal guest guest_nice
        read -r cpu user nice system idle iowait irq softirq steal guest guest_nice < /proc/stat
        
        # Calculate total and idle time
        cpu_total=$((user + nice + system + idle + iowait + irq + softirq + steal))
        cpu_idle=$idle
        
        # Calculate deltas
        delta_total=$((cpu_total - prev_cpu_total))
        delta_idle=$((cpu_idle - prev_cpu_idle))
        delta_iowait=$((iowait - prev_cpu_iowait))
        delta_steal=$((steal - prev_cpu_steal))
        
        if [ $delta_total -gt 0 ]; then
            cpu_usage_percent=$(awk "BEGIN {printf \"%.2f\", 100.0 * (1.0 - $delta_idle / $delta_total)}")
            cpu_iowait_percent=$(awk "BEGIN {printf \"%.2f\", 100.0 * $delta_iowait / $delta_total}")
            cpu_steal_percent=$(awk "BEGIN {printf \"%.2f\", 100.0 * $delta_steal / $delta_total}")
        fi
        
        prev_cpu_total=$cpu_total
        prev_cpu_idle=$cpu_idle
        prev_cpu_iowait=$iowait
        prev_cpu_steal=$steal
    elif [ -f /proc/stat ]; then
        # First sample - just record baseline
        read -r cpu user nice system idle iowait irq softirq steal guest guest_nice < /proc/stat
        prev_cpu_total=$((user + nice + system + idle + iowait + irq + softirq + steal))
        prev_cpu_idle=$idle
        prev_cpu_iowait=$iowait
        prev_cpu_steal=$steal
    fi
    
    # Memory info from /proc/meminfo
    if [ -f /proc/meminfo ]; then
        mem_total=$(awk '/MemTotal:/ {print $2}' /proc/meminfo)
        mem_available=$(awk '/MemAvailable:/ {print $2}' /proc/meminfo)
        swap_total=$(awk '/SwapTotal:/ {print $2}' /proc/meminfo)
        swap_free=$(awk '/SwapFree:/ {print $2}' /proc/meminfo)
        swap_used=$((swap_total - swap_free))
        
        # Convert KB to MB
        mem_total_mb=$((mem_total / 1024))
        mem_available_mb=$((mem_available / 1024))
        swap_total_mb=$((swap_total / 1024))
        swap_used_mb=$((swap_used / 1024))
        
        mem_percent=$(awk "BEGIN {printf \"%.2f\", 100.0 * (1.0 - $mem_available / $mem_total)}")
    else
        mem_total_mb="null"
        mem_available_mb="null"
        mem_percent="null"
        swap_total_mb="null"
        swap_used_mb="null"
    fi
    
    # Load average
    if [ -f /proc/loadavg ]; then
        load_1min=$(awk '{print $1}' /proc/loadavg)
        load_5min=$(awk '{print $2}' /proc/loadavg)
        load_15min=$(awk '{print $3}' /proc/loadavg)
    else
        load_1min="null"
        load_5min="null"
        load_15min="null"
    fi
    
    # Disk I/O from /proc/diskstats (interval-based, primary disk only)
    disk_read_mb_per_s="null"
    disk_write_mb_per_s="null"
    if [ -f /proc/diskstats ]; then
        # Find the primary block device (usually sda, nvme0n1, vda, or xvda)
        # Try to find mounted device for /
        primary_disk=$(df / | tail -1 | awk '{print $1}' | sed 's|/dev/||' | sed 's|[0-9]*$||')
        
        if [ -z "$primary_disk" ]; then
            # Fallback to first disk device
            primary_disk=$(lsblk -ndo NAME,TYPE 2>/dev/null | awk '$2=="disk" {print $1; exit}')
        fi
        
        if [ -n "$primary_disk" ] && [ $sample_count -gt 0 ]; then
            disk_device="\"$primary_disk\""
            disk_stats=$(awk -v disk="$primary_disk" '$3 == disk {print $6, $10}' /proc/diskstats)
            if [ -n "$disk_stats" ]; then
                read -r read_sectors write_sectors <<< "$disk_stats"
                
                # Calculate deltas (sectors) and convert to MB (assuming 512 bytes/sector)
                delta_read=$((read_sectors - prev_disk_read_sectors))
                delta_write=$((write_sectors - prev_disk_write_sectors))
                
                # Calculate actual time interval
                prev_sample_time=${prev_sample_time:-$current_time}
                actual_interval=$((current_time - prev_sample_time))
                if [ $actual_interval -le 0 ]; then
                    actual_interval=$INTERVAL
                fi
                
                # MB/s = (sectors * 512 bytes) / (1024*1024) / actual_interval
                disk_read_mb_per_s=$(awk "BEGIN {printf \"%.2f\", ($delta_read * 512.0) / (1024*1024) / $actual_interval}")
                disk_write_mb_per_s=$(awk "BEGIN {printf \"%.2f\", ($delta_write * 512.0) / (1024*1024) / $actual_interval}")
                
                prev_disk_read_sectors=$read_sectors
                prev_disk_write_sectors=$write_sectors
            fi
        elif [ -n "$primary_disk" ]; then
            # First sample - record baseline
            disk_device="\"$primary_disk\""
            disk_stats=$(awk -v disk="$primary_disk" '$3 == disk {print $6, $10}' /proc/diskstats)
            if [ -n "$disk_stats" ]; then
                read -r prev_disk_read_sectors prev_disk_write_sectors <<< "$disk_stats"
            fi
        fi
        
        prev_sample_time=$current_time
    fi
    
    # Write JSON sample
    cat > "$output_file" << EOF
{
  "timestamp": "$timestamp",
  "elapsed_seconds": $elapsed,
  "sample_number": $sample_count,
  "cpu": {
    "count": "$cpu_count",
    "usage_percent": $cpu_usage_percent,
    "iowait_percent": $cpu_iowait_percent,
    "steal_percent": $cpu_steal_percent,
    "load_1min": $load_1min,
    "load_5min": $load_5min,
    "load_15min": $load_15min
  },
  "memory": {
    "total_mb": $mem_total_mb,
    "available_mb": $mem_available_mb,
    "used_percent": $mem_percent
  },
  "swap": {
    "total_mb": $swap_total_mb,
    "used_mb": $swap_used_mb
  },
  "disk_io": {
    "device": $disk_device,
    "read_mb_per_s": $disk_read_mb_per_s,
    "write_mb_per_s": $disk_write_mb_per_s
  }
}
EOF
    
    sample_count=$((sample_count + 1))
    
    sleep "$INTERVAL"
done

echo "[CI Monitor] Sampler stopped after $sample_count samples"
rm -f "$PID_FILE"
