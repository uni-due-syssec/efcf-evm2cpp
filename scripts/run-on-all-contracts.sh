#!/bin/sh
set -ex

cd contracts
make all
cd ..

export RUST_BACKTRACE=1
for x in ./contracts/*.combined.json; do 
    echo "$x"; 
    cargo run "$(basename "$x" | cut -d '.' -f 1)" "$x"; 
done
