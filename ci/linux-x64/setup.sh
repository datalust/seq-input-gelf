#!/bin/bash

sudo apt-get update && sudo apt-get install -y libnss3-tools --no-install-recommends

chmod +x ./tool/mkcert-linux-x64
./tool/mkcert-linux-x64 -install

RequiredRustToolchain=$(cat ./rust-toolchain)

curl https://sh.rustup.rs -sSf | sh -s -- --default-host x86_64-unknown-linux-gnu --default-toolchain $RequiredRustToolchain -y

export PATH="$HOME/.cargo/bin:$PATH"

cargo install -f cross

docker run --privileged --rm docker/binfmt:a7996909642ee92942dcd6cff44b9b95f08dad64
sudo service docker restart

ls /home/appveyor
ls /home/appveyor/.cargo
