#!/bin/bash
set -uex

DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" >/dev/null 2>&1 && pwd )"
cd $DIR/../..

# make sure submodule is correctly cloned!!
git submodule update --init --recursive
git pull origin master --recurse-submodules --ff-only
cargo run --release --bin gen_global_state_testcase

cd $DIR/../../circuits; npm i
snarkit --version || npm -g install snarkit
snarkit test testdata/Block_2_2_7_2/ --backend=wasm
