#!/bin/bash
set -uex

DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" >/dev/null 2>&1 && pwd )"
REPO_DIR=$DIR/"../.."
cd $REPO_DIR

# make sure submodule is correctly cloned!!
git submodule update --init --recursive
if [ -z ${CI+x} ]; then git pull --recurse-submodules; fi
cargo run --release --bin gen_export_circuit_testcase

cd $REPO_DIR/circuits; npm i
snarkit --version || npm -g install snarkit
snarkit test testdata/Block_2_2_2_2/ --force_recompile --backend=wasm
snarkit test testdata/CheckLeafUpdate_2/ --force_recompile --backend=wasm
