FROM docker.io/archlinux:base-devel

RUN pacman -Syu --noconfirm --needed \
  && pacman-db-upgrade \
  && pacman -Syu --noconfirm --needed \
      git wget curl unzip bash jq python python-pip \
      clang llvm lld libc++ \
      meson cmake ninja \
      libunwind binutils \
      rust rust-analyzer \
  && pacman -Scc --noconfirm

# first we install honggfuzz and AFL++

RUN mkdir -p /src/
WORKDIR /src/

ARG AFLPP_VERSION=dev
RUN git clone -b $AFLPP_VERSION --depth=1 https://github.com/AFLplusplus/AFLplusplus.git \
  && pushd AFLplusplus \
  && make source-only \
  && make install

ARG HFUZZ_VERSION=2.3.1
RUN git clone -b $HFUZZ_VERSION --depth=1 https://github.com/google/honggfuzz \
  && cd honggfuzz \
  && make && make install

env PATH=$PATH:/usr/local/bin/

# we install all kinds of solidity versions
WORKDIR /src/
RUN pip install solc-select
RUN solc-select install all
ENV PATH="/root/.solc-select/artifacts/:${PATH}"

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
