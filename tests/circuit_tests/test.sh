#!/bin/bash
set -uex

export NTXS=2;
export BALANCELEVELS=2;
export ORDERLEVELS=2;
export ACCOUNTLEVELS=2;
export VERBOSE=false;

DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" >/dev/null 2>&1 && pwd )"
REPO_DIR=$DIR/"../.."
cd $REPO_DIR

# make sure submodule is correctly cloned!!
git submodule update --init --recursive
if [ -z ${CI+x} ]; then git pull --recurse-submodules; fi
cargo run --bin gen_export_circuit_testcase # debug mode for fast compile

cd $REPO_DIR/circuits
if [ -z ${CI+x} ]; then npm ci; fi
snarkit --version || npm -g install snarkit
snarkit test testdata/Block_$NTXS"_"$BALANCELEVELS"_"$ORDERLEVELS"_"$ACCOUNTLEVELS/ --force_recompile --backend=wasm
snarkit test testdata/CheckLeafUpdate_2/ --force_recompile --backend=wasm
