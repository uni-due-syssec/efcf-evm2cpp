#!/usr/bin/env bash

set -eu -o pipefail
set -x

export DEBIAN_FRONTEND=noninteractive
apt-get update -q &&
    apt-get full-upgrade -q -y &&
    apt-get install -q -y \
        git wget curl unzip subversion \
        build-essential cmake meson ninja-build automake autoconf texinfo flex bison pkg-config \
        binutils-multiarch binutils-multiarch-dev \
        libfontconfig1-dev libgraphite2-dev libharfbuzz-dev libicu-dev libssl-dev zlib1g-dev \
        libtool-bin python3-dev libglib2.0-dev libpixman-1-dev clang python3-setuptools llvm \
        python3 python3-dev python3-pip python-is-python3 \
        gcc-multilib gcc-10-multilib \
        libunwind-dev libunwind8 \
        gcc-10-plugin-dev \
        bash \
        llvm clang llvm-dev llvm-tools lld clang-format \
        libc++1 libc++-dev libc++abi1 libc++abi-dev \
        llvm-11 clang-11 llvm-11-dev llvm-11-tools lld-11 clang-format-11 \
        libc++1-11 libc++-11-dev libc++abi1-11 libc++abi-11-dev \
        jq &&
    apt-get clean -y &&
    rm -rf /var/lib/apt/lists/*

if ! command -v rustup; then
    echo "installing rustup!"
    wget -q -O /tmp/rustup.sh https://sh.rustup.rs && sh /tmp/rustup.sh -y
    echo "make sure to set your PATH to contains '\$HOME/.cargo/bin'"
    echo "or to 'source \"\$HOME/.cargo/env\"'"
    cat >>"$HOME/.profile" <<EOF
source "\$HOME/.cargo/env"
EOF
    cat >>"$HOME/.bashrc" <<EOF
source "\$HOME/.cargo/env"
EOF
fi
source "$HOME/.cargo/env"

mkdir -p /src/
cd /src/

export AFLPP_VERSION=stable
git clone -b $AFLPP_VERSION --depth=1 https://github.com/AFLplusplus/AFLplusplus.git &&
    cd AFLplusplus &&
    make source-only &&
    make install

cd /src/
export HFUZZ_VERSION=2.4
git clone -b $HFUZZ_VERSION --depth=1 https://github.com/google/honggfuzz &&
    cd honggfuzz &&
    make && make install

# we install all kinds of solidity versions
pip3 install solc-select
solc-select install all
