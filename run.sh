#! /bin/bash

qemu-system-x86_64 -drive \
    format=raw,file=target/x86_64-build-target/debug/bootimage-ferros.bin \
    -monitor stdio
