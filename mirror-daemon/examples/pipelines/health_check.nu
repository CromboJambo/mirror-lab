#!/usr/bin/env nu
# System health check pipeline
# Demonstrates basic system inspection

let timestamp = date now | format date "%Y-%m-%d %H:%M:%S"

let health_report = {
    timestamp: $timestamp,
    hostname: (sys host | get hostname),
    kernel: (sys host | get kernel_version),
    uptime: (sys host | get uptime),
    cpu_count: (sys cpu | length),
    memory_total: (sys mem | get total),
    memory_available: (sys mem | get available),
}

$health_report | to json | save health_check.json

print "Health Check Complete"
print $"Timestamp: ($health_report.timestamp)"
print $"Hostname: ($health_report.hostname)"
print $"CPU cores: ($health_report.cpu_count)"
