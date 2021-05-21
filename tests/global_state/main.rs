#![allow(dead_code)]
#![allow(clippy::upper_case_acronyms)]
#![allow(clippy::large_enum_variant)]

/*
 * have a look at scripts/global_state_test.sh
 */

mod bench;
mod gen_testcase;
mod types;
use gen_testcase::run;

#[cfg(feature = "bench_global_state")]
use bench::run_bench;

fn main() {
    match run() {
        Ok(_) => println!("global_state test_case generated"),
        Err(e) => panic!("{:#?}", e),
    }

    #[cfg(feature = "bench_global_state")]
    run_bench().expect("bench ok");
}
