use lazy_static::lazy_static;

lazy_static! {
    pub static ref NTXS: usize = std::env::var("NTXS")
        .expect("NTXS not set in ENV")
        .parse::<usize>()
        .expect("parse NTXS");
    pub static ref BALANCELEVELS: usize = std::env::var("BALANCELEVELS")
        .expect("BALANCELEVELS not set in ENV")
        .parse::<usize>()
        .expect("parse BALANCELEVELS");
    pub static ref ORDERLEVELS: usize = std::env::var("ORDERLEVELS")
        .expect("ORDERLEVELS not set in ENV")
        .parse::<usize>()
        .expect("parse ORDERLEVELS");
    pub static ref ACCOUNTLEVELS: usize = std::env::var("ACCOUNTLEVELS")
        .expect("ACCOUNTLEVELS not set in ENV")
        .parse::<usize>()
        .expect("parse ACCOUNTLEVELS");
    pub static ref MAXORDERNUM: usize = 2usize.pow(*ORDERLEVELS as u32);
    pub static ref MAXACCOUNTNUM: usize = 2usize.pow(*ACCOUNTLEVELS as u32);
    pub static ref MAXTOKENNUM: usize = 2usize.pow(*BALANCELEVELS as u32);
    pub static ref VERBOSE: bool = std::env::var("VERBOSE")
        .unwrap_or_else(|_| false.to_string())
        .parse::<bool>()
        .unwrap_or(false);

    // default overwrite for now
    pub static ref OVERWRITE_SIGNATURE: bool = std::env::var("OVERWRITE_SIGNATURE")
        .unwrap_or_else(|_| true.to_string())
        .parse::<bool>()
        .unwrap_or(true);
}
