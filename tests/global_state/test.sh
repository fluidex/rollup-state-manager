#!/bin/bash
set -uex

export NTXS=2;
export BALANCELEVELS=2;
export ORDERLEVELS=4;
export ACCOUNTLEVELS=4;
export VERBOSE=false;

export RUST_BACKTRACE=full

DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" >/dev/null 2>&1 && pwd )"
REPO_DIR=$DIR/"../.."
cd $REPO_DIR

# make sure submodule is correctly cloned!!
# git submodule update --init --recursive
# if [ -z ${CI+x} ]; then git pull --recurse-submodules; fi
cargo run --features="persist_sled, fr_string_repr" --bin gen_global_state_testcase -- tests/global_state/testdata/data001.txt # debug mode for fast compile

cd $REPO_DIR/circuits; npm i
snarkit --version || npm -g install snarkit
snarkit test testdata/Block_$NTXS"_"$BALANCELEVELS"_"$ORDERLEVELS"_"$ACCOUNTLEVELS/ --force_recompile --backend=wasm
