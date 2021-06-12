#!/bin/bash
set -uex

export NTXS=2;
export BALANCELEVELS=2;
export ORDERLEVELS=3;
export ACCOUNTLEVELS=2;
export VERBOSE=false;

export RUST_BACKTRACE=full

DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" >/dev/null 2>&1 && pwd )"
REPO_DIR=$DIR/"../.."
cd $REPO_DIR

if [ -z ${CI+x} ]; then 
	cargo run --bin gen_global_state_testcase #tests/global_state/testdata/data001.txt
	#cargo run --bin --release gen_global_state_testcase
else
	# make sure submodule is correctly cloned!!
	git submodule update --init --recursive
	#git pull --recurse-submodules
	cargo run --bin gen_global_state_testcase # debug mode for fast compile
fi


cd $REPO_DIR/circuits; npm i
snarkit --version || npm -g install snarkit
snarkit test testdata/Block_$NTXS"_"$BALANCELEVELS"_"$ORDERLEVELS"_"$ACCOUNTLEVELS/ --force_recompile --backend=wasm
