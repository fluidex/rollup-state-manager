#!/bin/bash
set -uex

DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" >/dev/null 2>&1 && pwd )"
cd $DIR/../..

# make sure submodule is correctly cloned!!
git submodule update --init --recursive
git pull origin master --recurse-submodules
cargo run --release --bin gen_export_circuit_testcase

cd $DIR/../../circuits; npm i
snarkit --version || npm -g install snarkit
snarkit test testdata/CheckLeafUpdate_2/ --backend=wasm
