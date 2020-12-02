#!/usr/bin/env bash
set -ex -o pipefail

pushd contracts
make all
popd

export RUST_BACKTRACE=1
#export AFL_DEBUG=1
for x in ./contracts/*.combined.json; do 
    echo "$x"; 
    contract_name="$(basename "$x" | cut -d '.' -f 1)"
    echo "[+] running evm2cpp"
    cargo run "$contract_name" "$x";

    echo "[+] Building contract $contract_name"
    pushd eEVM
    ./quick-build.sh hfuzz "$contract_name"

    echo "[+] resetting eEVM repo"
    git reset --hard
    rm -rf contracts include
    git checkout .
    popd
done
