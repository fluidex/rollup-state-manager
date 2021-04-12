pub const NTXS: usize = 2;
pub const BALANCELEVELS: usize = 2;
pub const ORDERLEVELS: usize = 7;
pub const ACCOUNTLEVELS: usize = 2;
pub const MAXORDERNUM: usize = 2usize.pow(ORDERLEVELS as u32);
pub const MAXACCOUNTNUM: usize = 2usize.pow(ACCOUNTLEVELS as u32);
pub const MAXTOKENNUM: usize = 2usize.pow(BALANCELEVELS as u32);
pub const VERBOSE: bool = false;

// TODO: enum, from?
pub fn token_id(token_name: &str) -> u32 {
    match token_name {
        "ETH" => 0,
        "USDT" => 1,
        _ => unreachable!(),
    }
}

// TODO: enum, impl?
pub fn prec(token_id: u32) -> u32 {
    match token_id {
        0 | 1 => 6,
        _ => unreachable!(),
    }
}
