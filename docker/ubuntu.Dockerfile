# groovy has llvm-11 as default; so this works out
ARG UBUNTU_VERSION=groovy
FROM docker.io/ubuntu:$UBUNTU_VERSION

ARG DEBIAN_FRONTEND=noninteractive
RUN apt-get update -q \
  && apt-get full-upgrade -q -y \
  && apt-get install -q -y \
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
    llvm-11 clang-11 llvm-11-dev llvm-11-tools lld-11 clang-format-11 \
    libc++1 libc++-dev libc++1-11 libc++-11-dev libc++abi1-11 libc++abi-11-dev \
    cargo rustc \
    jq \
  && apt-get clean -y \
  && rm -rf /var/lib/apt/lists/*


# first we install honggfuzz and AFL++

RUN mkdir -p /src/
WORKDIR /src/

ARG AFLPP_VERSION=stable
RUN git clone -b $AFLPP_VERSION --depth=1 https://github.com/AFLplusplus/AFLplusplus.git \
  && cd AFLplusplus \
  && make source-only \
  && make install

ARG HFUZZ_VERSION=2.3.1
RUN git clone -b $HFUZZ_VERSION --depth=1 https://github.com/google/honggfuzz \
  && cd honggfuzz \
  && make && make install

env PATH=$PATH:/usr/local/bin/

# we install all kinds of solidity versions
WORKDIR /src/
RUN pip3 install solc-select
ENV PATH="${PATH}:/root/.local/bin/:/root/.solc-select/artifacts/"
RUN solc-select install all

WORKDIR /

# we add the source from here directly to the image

RUN mkdir -p /app/
COPY . /app/
WORKDIR /app/

# and we also build it
RUN cargo build --release

#RUN cd eEVM/ \
#  && ./quick-build.sh \
#  && ./quick-build.sh hfuzz \
#  && ./quick-build.sh afuzz 
