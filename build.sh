#!/bin/sh

# Set the hostname for the remarkable based on $1 and fall back to `remarkable`
remarkable="${1:-remarkable}"

cross build --release --target=armv7-unknown-linux-gnueabihf && scp target/armv7-unknown-linux-gnueabihf/release/ghostwriter root@$remarkable:

