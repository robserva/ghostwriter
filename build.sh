#!/bin/sh

cross build --release --target=armv7-unknown-linux-gnueabihf && scp target/armv7-unknown-linux-gnueabihf/release/ghostwriter remarkable:
