#!/bin/bash
set -uex

NTXS=2;
BALANCELEVELS=2;
ORDERLEVELS=3;
ACCOUNTLEVELS=2;

DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" >/dev/null 2>&1 && pwd )"
REPO_DIR=$DIR/"../.."
cd $REPO_DIR

# make sure submodule is correctly cloned!!
git submodule update --init --recursive
if [ -z ${CI+x} ]; then git pull --recurse-submodules; fi
cargo run --release --bin gen_global_state_testcase

cd $REPO_DIR/circuits; npm i
snarkit --version || npm -g install snarkit
snarkit test testdata/Block_$NTXS_$BALANCELEVELS_$ORDERLEVELS_$ACCOUNTLEVELS/ --force_recompile --backend=wasm
