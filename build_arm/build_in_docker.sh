#!/bin/bash -ex

cd "$(dirname "$0")/../"

PATH=/root/.cargo/bin:$PATH

export CARGO_TARGET_DIR="$(dirname "$0")/target/"
mkdir -p "$CARGO_TARGET_DIR"

cargo build \
      --target=armv7-unknown-linux-gnueabihf \
      --config target.armv7-unknown-linux-gnueabihf.linker=\"arm-linux-gnueabihf-gcc\" \
      --no-default-features \
      --release --example showimg
