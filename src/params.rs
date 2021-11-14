use itertools::sorted;
use lazy_static::lazy_static;
use std::env;
use std::fmt::Debug;
use std::str::FromStr;

lazy_static! {
    pub static ref TX_SLOT_NUMS: Vec<usize> = sorted(parse_env_to_collection::<Vec<usize>, usize>("TX_SLOT_NUMS")).collect();
    pub static ref BALANCELEVELS: usize = env::var("BALANCELEVELS")
        .expect("BALANCELEVELS not set in ENV")
        .parse::<usize>()
        .expect("parse BALANCELEVELS");
    pub static ref ORDERLEVELS: usize = env::var("ORDERLEVELS")
        .expect("ORDERLEVELS not set in ENV")
        .parse::<usize>()
        .expect("parse ORDERLEVELS");
    pub static ref ACCOUNTLEVELS: usize = env::var("ACCOUNTLEVELS")
        .expect("ACCOUNTLEVELS not set in ENV")
        .parse::<usize>()
        .expect("parse ACCOUNTLEVELS");
    pub static ref MAXORDERNUM: usize = 2usize.pow(*ORDERLEVELS as u32);
    pub static ref MAXACCOUNTNUM: usize = 2usize.pow(*ACCOUNTLEVELS as u32);
    pub static ref MAXTOKENNUM: usize = 2usize.pow(*BALANCELEVELS as u32);
    pub static ref VERBOSE: bool = env::var("VERBOSE")
        .unwrap_or_else(|_| false.to_string())
        .parse::<bool>()
        .unwrap_or(false);

    // default overwrite for now
    pub static ref OVERWRITE_SIGNATURE: bool = env::var("OVERWRITE_SIGNATURE")
        .unwrap_or_else(|_| true.to_string())
        .parse::<bool>()
        .unwrap_or(true);
}

fn parse_env_to_collection<F, I>(name: &str) -> F
where
    I: FromStr,
    I::Err: Debug,
    F: FromIterator<I>,
{
    env::var(name)
        .unwrap_or_else(|_| panic!("{} not set in ENV", name))
        .split(',')
        .map(|i| i.trim().parse::<I>().unwrap())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_env_to_collection() {
        env::set_var("DUMMY_USIZE_ARRAY", "2, 16, 64,512");
        let parse_result: Vec<usize> = parse_env_to_collection("DUMMY_USIZE_ARRAY");
        assert_eq!(parse_result, [2, 16, 64, 512]);
    }
}
