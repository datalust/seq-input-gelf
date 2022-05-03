#!/bin/bash

RequiredRustToolchain=$(cat ./rust-toolchain)

curl https://sh.rustup.rs -sSf | sh -s -- --default-host x86_64-unknown-linux-gnu --default-toolchain $RequiredRustToolchain -y

export PATH="$HOME/.cargo/bin:$PATH"

rustup target add x86_64-unknown-linux-musl
rustup target add aarch64-unknown-linux-musl

docker run --privileged --rm docker/binfmt:a7996909642ee92942dcd6cff44b9b95f08dad64
sudo service docker restart

ls /home/appveyor
ls /home/appveyor/.cargo
