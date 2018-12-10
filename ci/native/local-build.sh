#! /bin/bash

set -e

cargo clean

if [ "$SQELF_TEST" = "0" ] || [ "$SQELF_NATIVE_TEST" = "0" ]; then
    echo "Ignoring tests"
else
    echo "Running tests"

    cargo test --target x86_64-unknown-linux-gnu
fi

cargo build --target x86_64-unknown-linux-gnu --release
