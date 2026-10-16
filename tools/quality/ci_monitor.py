#!/usr/bin/env python3
"""CI performance monitoring and profiling utility."""

from __future__ import annotations

import json
import os
import platform
import subprocess
import time
from dataclasses import dataclass, asdict
from datetime import datetime
from pathlib import Path
from typing import Any


@dataclass
class SystemSnapshot:
    """System resource snapshot at a point in time."""
    timestamp: str
    cpu_count: int
    cpu_percent: float | None
    memory_total_mb: int
    memory_available_mb: int
    memory_percent: float
    swap_total_mb: int
    swap_used_mb: int
    disk_io_read_mb: float | None
    disk_io_write_mb: float | None


@dataclass
class EnvironmentInfo:
    """CI environment and tool version information."""
    workflow_sha: str
    base_sha: str | None
    head_sha: str | None
    runner_os: str
    runner_arch: str
    runner_image_os: str | None
    runner_image_version: str | None
    cpu_model: str | None
    memory_total_mb: int
    rust_version: str
    cargo_version: str
    nextest_version: str | None
    llvm_cov_version: str | None
    measurer_binary: str | None
    measurer_sha256: str | None
    python_version: str
    ci_parallel_risk: bool
    test_threads: str
    target_dir_size: str | None


@dataclass
class TimingEvent:
    """Individual timing event marker."""
    name: str
    timestamp: str
    elapsed_since_start: float


class CIMonitor:
    """Monitor CI performance metrics."""
    
    def __init__(self, output_dir: Path):
        self.output_dir = Path(output_dir)
        self.output_dir.mkdir(parents=True, exist_ok=True)
        self.start_time = time.time()
        self.timing_events: list[TimingEvent] = []
        
    def mark_event(self, name: str) -> None:
        """Record a timing event."""
        now = time.time()
        event = TimingEvent(
            name=name,
            timestamp=datetime.now().isoformat(),
            elapsed_since_start=now - self.start_time
        )
        self.timing_events.append(event)
        print(f"[TIMING] {name} at {event.elapsed_since_start:.2f}s")
        
    def collect_environment_info(self) -> EnvironmentInfo:
        """Collect environment and tool version information."""
        
        def get_version(command: list[str]) -> str | None:
            try:
                result = subprocess.run(
                    command,
                    capture_output=True,
                    text=True,
                    timeout=5
                )
                return result.stdout.strip() if result.returncode == 0 else None
            except Exception:
                return None
        
        # Get CPU model and memory
        cpu_model = None
        memory_total_mb = 0
        try:
            if platform.system() == "Linux":
                with open("/proc/cpuinfo") as f:
                    for line in f:
                        if line.startswith("model name"):
                            cpu_model = line.split(":", 1)[1].strip()
                            break
                
                with open("/proc/meminfo") as f:
                    for line in f:
                        if line.startswith("MemTotal:"):
                            memory_total_mb = int(line.split()[1]) // 1024
                            break
        except Exception:
            pass
        
        # Get Rust toolchain versions
        rust_version = get_version(["rustc", "--version"]) or "unknown"
        cargo_version = get_version(["cargo", "--version"]) or "unknown"
        nextest_version = get_version(["cargo", "nextest", "--version"])
        llvm_cov_version = get_version(["cargo", "llvm-cov", "--version"])
        
        # Get measurer binary checksum from the info file written by ci_quality.py
        measurer_sha256 = None
        measurer_path_str = None
        measurer_info_file = Path("target/ci-monitor/measurer_info.txt")
        if measurer_info_file.exists():
            try:
                lines = measurer_info_file.read_text().strip().split('\n')
                if len(lines) >= 2:
                    measurer_path_str = lines[0]
                    measurer_sha256 = lines[1]  # Full 64-char SHA256
            except Exception:
                pass
        
        # Get target directory size
        target_dir_size = None
        try:
            result = subprocess.run(["du", "-sh", "target"], 
                                  capture_output=True, text=True, timeout=5)
            if result.returncode == 0:
                target_dir_size = result.stdout.split()[0]
        except Exception:
            pass
        
        return EnvironmentInfo(
            workflow_sha=os.environ.get("GITHUB_SHA", "unknown"),
            base_sha=os.environ.get("BASE_SHA"),
            head_sha=os.environ.get("HEAD_SHA"),
            runner_os=platform.system(),
            runner_arch=platform.machine(),
            runner_image_os=os.environ.get("ImageOS"),
            runner_image_version=os.environ.get("ImageVersion"),
            cpu_model=cpu_model,
            memory_total_mb=memory_total_mb,
            rust_version=rust_version,
            cargo_version=cargo_version,
            nextest_version=nextest_version,
            llvm_cov_version=llvm_cov_version,
            measurer_binary=measurer_path_str,
            measurer_sha256=measurer_sha256,
            python_version=platform.python_version(),
            ci_parallel_risk=os.environ.get("CI_PARALLEL_RISK") == "1",
            test_threads=os.environ.get("NEXTEST_TEST_THREADS", "default"),
            target_dir_size=target_dir_size
        )
    
    def collect_system_snapshot(self) -> SystemSnapshot:
        """Collect current system resource usage."""
        
        # Get memory info from /proc/meminfo
        memory_total = memory_available = swap_total = swap_used = 0
        try:
            with open("/proc/meminfo") as f:
                mem_info = {}
                for line in f:
                    parts = line.split()
                    if len(parts) >= 2:
                        mem_info[parts[0].rstrip(":")] = int(parts[1])
                
                memory_total = mem_info.get("MemTotal", 0) // 1024  # Convert to MB
                memory_available = mem_info.get("MemAvailable", 0) // 1024
                swap_total = mem_info.get("SwapTotal", 0) // 1024
                swap_free = mem_info.get("SwapFree", 0) // 1024
                swap_used = swap_total - swap_free
        except Exception:
            pass
        
        memory_percent = 0.0
        if memory_total > 0:
            memory_percent = 100.0 * (1.0 - memory_available / memory_total)
        
        # Get CPU count
        cpu_count = os.cpu_count() or 1
        
        # Get CPU usage (simple approximation using /proc/stat)
        cpu_percent = None
        try:
            with open("/proc/stat") as f:
                line = f.readline()
                if line.startswith("cpu "):
                    fields = line.split()[1:]
                    total = sum(int(x) for x in fields)
                    idle = int(fields[3])
                    cpu_percent = 100.0 * (1.0 - idle / total) if total > 0 else 0.0
        except Exception:
            pass
        
        # Get disk I/O stats
        disk_io_read = disk_io_write = None
        try:
            with open("/proc/diskstats") as f:
                # Sum all devices (simplified)
                total_read_sectors = 0
                total_write_sectors = 0
                for line in f:
                    parts = line.split()
                    if len(parts) >= 14:
                        # Read sectors are at index 5, write sectors at index 9
                        total_read_sectors += int(parts[5])
                        total_write_sectors += int(parts[9])
                
                # Convert sectors to MB (assuming 512 byte sectors)
                disk_io_read = total_read_sectors * 512 / (1024 * 1024)
                disk_io_write = total_write_sectors * 512 / (1024 * 1024)
        except Exception:
            pass
        
        return SystemSnapshot(
            timestamp=datetime.now().isoformat(),
            cpu_count=cpu_count,
            cpu_percent=cpu_percent,
            memory_total_mb=memory_total,
            memory_available_mb=memory_available,
            memory_percent=memory_percent,
            swap_total_mb=swap_total,
            swap_used_mb=swap_used,
            disk_io_read_mb=disk_io_read,
            disk_io_write_mb=disk_io_write
        )
    
    def save_environment_info(self) -> None:
        """Save environment information to file."""
        env_info = self.collect_environment_info()
        output_file = self.output_dir / "environment.json"
        
        with open(output_file, "w") as f:
            json.dump(asdict(env_info), f, indent=2)
        
        print(f"[MONITOR] Environment info saved to {output_file}")
        print(f"  Workflow: {env_info.workflow_sha[:8]}")
        if env_info.base_sha:
            print(f"  Base SHA: {env_info.base_sha[:8]}")
        if env_info.head_sha:
            print(f"  Head SHA: {env_info.head_sha[:8]}")
        print(f"  Rust: {env_info.rust_version}")
        print(f"  Runner: {env_info.runner_os} {env_info.runner_arch}")
        if env_info.runner_image_os:
            print(f"  Image: {env_info.runner_image_os} {env_info.runner_image_version}")
        if env_info.cpu_model:
            print(f"  CPU: {env_info.cpu_model}")
        print(f"  Memory: {env_info.memory_total_mb}MB")
        print(f"  Parallel Risk: {env_info.ci_parallel_risk}")
        print(f"  Test Threads: {env_info.test_threads}")
        if env_info.target_dir_size:
            print(f"  Target dir size: {env_info.target_dir_size}")
        if env_info.measurer_sha256:
            print(f"  Measurer: {env_info.measurer_sha256[:16]}...")
    
    def save_system_snapshot(self, name: str = "snapshot") -> None:
        """Save a single system resource snapshot."""
        snapshot = self.collect_system_snapshot()
        output_file = self.output_dir / f"{name}.json"
        
        with open(output_file, "w") as f:
            json.dump(asdict(snapshot), f, indent=2)
        
        print(f"[MONITOR] System snapshot saved: {name}")
        print(f"  Memory: {snapshot.memory_percent:.1f}% used "
              f"({snapshot.memory_available_mb}MB available / {snapshot.memory_total_mb}MB total)")
        print(f"  CPU count: {snapshot.cpu_count}")
    
    def save_timing_events(self) -> None:
        """Save all timing events."""
        output_file = self.output_dir / "timing_events.json"
        
        with open(output_file, "w") as f:
            json.dump([asdict(e) for e in self.timing_events], f, indent=2)
        
        print(f"[MONITOR] Timing events saved to {output_file}")
        print(f"  Total events: {len(self.timing_events)}")
        if self.timing_events:
            print(f"  Duration: {self.timing_events[-1].elapsed_since_start:.2f}s")


def main():
    """CLI entry point."""
    import argparse
    
    parser = argparse.ArgumentParser(description="CI performance monitoring")
    parser.add_argument("--output-dir", type=Path, required=True,
                       help="Output directory for monitoring data")
    parser.add_argument("--command", choices=["env", "snapshot", "mark"],
                       required=True, help="Monitoring command")
    parser.add_argument("--name", help="Name for snapshot or timing mark")
    
    args = parser.parse_args()
    
    monitor = CIMonitor(args.output_dir)
    
    if args.command == "env":
        monitor.save_environment_info()
    elif args.command == "snapshot":
        monitor.save_system_snapshot(args.name or "snapshot")
    elif args.command == "mark":
        if not args.name:
            parser.error("--name is required for mark command")
        monitor.mark_event(args.name)
        monitor.save_timing_events()


if __name__ == "__main__":
    main()
