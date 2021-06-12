#!/bin/bash
set -uex

export NTXS=2;
export BALANCELEVELS=20;
export ORDERLEVELS=20;
export ACCOUNTLEVELS=20;
export VERBOSE=false;

DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" >/dev/null 2>&1 && pwd )"
REPO_DIR=$DIR/"../.."
cd $REPO_DIR

if [ -z ${CI+x} ]; then 
	#git pull --recurse-submodules; 
	echo not CI mode > /dev/null
else
	# make sure submodule is correctly cloned!!
	git submodule update --init --recursive
fi
cargo run --release --bin bench_global_state
