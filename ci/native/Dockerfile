FROM ubuntu:18.04

RUN apt-get update && \
    apt-get install \
       ca-certificates \
       curl \
       gcc \
       libc6-dev \
       -qqy \
       --no-install-recommends \
    && rm -rf /var/lib/apt/lists/*

ARG TOOLCHAIN=stable
ENV RUSTUP_URL=https://sh.rustup.rs
ENV RUSTUP_HOME=/rust/rustup
ENV CARGO_HOME=/rust/cargo
ENV PATH "$PATH:/rust/cargo/bin"

RUN curl $RUSTUP_URL -sSf | sh -s -- -y --default-toolchain $TOOLCHAIN
