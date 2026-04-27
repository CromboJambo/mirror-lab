#!/usr/bin/env nu
# Simple cashflow report generator
# This demonstrates basic output creation

let data = [
    [month revenue expenses profit];
    [Jan 10000 8000 2000]
    [Feb 12000 9000 3000]
    [Mar 11000 8500 2500]
    [Apr 13000 9500 3500]
]

# Save as JSON
$data | to json | save cashflow.json

# Save as CSV
$data | to csv | save cashflow.csv

# Print summary
print "Cashflow Report Generated"
print $"Total months: ($data | length)"
print $"Total revenue: ($data | get revenue | math sum)"
print $"Total expenses: ($data | get expenses | math sum)"
print $"Total profit: ($data | get profit | math sum)"
