#!/usr/bin/env fish

sudo setenforce 0
cross build --release --target aarch64-unknown-linux-gnu || echo "Note: Remember to disable the linker flag in ~/.cargo/config.toml"
