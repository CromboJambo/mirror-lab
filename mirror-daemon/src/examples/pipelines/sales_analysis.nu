#!/usr/bin/env nu
# Data transformation pipeline
# Demonstrates filtering and aggregation

let sales_data = [
    [product region sales];
    [Widget A 1000]
    [Widget B 1500]
    [Gadget A 2000]
    [Gadget B 1800]
    [Widget A 1200]
    [Gadget A 2200]
]

# Aggregate by product
let by_product = $sales_data 
    | group-by product 
    | transpose product data
    | insert total_sales { |row| $row.data | get sales | math sum }
    | select product total_sales

# Aggregate by region
let by_region = $sales_data
    | group-by region
    | transpose region data
    | insert total_sales { |row| $row.data | get sales | math sum }
    | select region total_sales

# Save results
$by_product | to json | save sales_by_product.json
$by_region | to json | save sales_by_region.json

print "Sales Analysis Complete"
print $"Products analyzed: ($by_product | length)"
print $"Regions analyzed: ($by_region | length)"
