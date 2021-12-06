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

# make sure submodule is correctly cloned!!
git submodule update --init --recursive
if [ -z ${CI+x} ]; then git pull --recurse-submodules; fi

cargo run --no-default-features --features="profiling" --release --bin bench_global_state
if command -v ~/go/bin/pprof &> /dev/null
then
    ~/go/bin/pprof -svg profile.pb
else
    echo "command pprof could not be found, needs to be installed via https://github.com/google/pprof"
fi
