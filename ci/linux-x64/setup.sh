#!/bin/bash

sudo apt-get update && sudo apt-get install -y libnss3-tools --no-install-recommends

chmod +x ./tool/mkcert-linux-x64
./tool/mkcert-linux-x64 -install

RequiredRustToolchain=$(cat ./rust-toolchain)

curl https://sh.rustup.rs -sSf | sh -s -- --default-host x86_64-unknown-linux-gnu --default-toolchain $RequiredRustToolchain -y

export PATH="$HOME/.cargo/bin:$PATH"

cargo install -f cross

docker run --privileged --rm linuxkit/binfmt:bebbae0c1100ebf7bf2ad4dfb9dfd719cf0ef132
sudo service docker restart

ls /home/appveyor
ls /home/appveyor/.cargo
