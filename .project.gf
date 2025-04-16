[gdb]
path=./rust-gdb

[commands]
Compile=shell cargo b --bin food-inventory-sim-tool --profile debugging
Run=file target/debugging/food-inventory-sim-tool;run&