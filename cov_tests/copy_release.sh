#!/bin/bash

cd ..

echo "Building blkar"
cargo build --release

echo "Copying blkar binary over"
cp target/release/blkar ./tests/blkar
cd tests
