use lazy_static::lazy_static;

lazy_static! {
    pub static ref NTXS: usize = std::env::var("NTXS").unwrap().parse::<usize>().unwrap();
}

// pub static NTXS: usize = std::env::var("NTXS").unwrap().parse::<usize>().unwrap();
// pub const NTXS: usize = 2;

pub const BALANCELEVELS: usize = 2;
pub const ORDERLEVELS: usize = 3;
pub const ACCOUNTLEVELS: usize = 2;
/*

      pub const BALANCELEVELS: usize = 20;
      pub const ORDERLEVELS: usize = 20;
      pub const ACCOUNTLEVELS: usize = 20;
*/
pub const MAXORDERNUM: usize = 2usize.pow(ORDERLEVELS as u32);
pub const MAXACCOUNTNUM: usize = 2usize.pow(ACCOUNTLEVELS as u32);
pub const MAXTOKENNUM: usize = 2usize.pow(BALANCELEVELS as u32);
pub const VERBOSE: bool = false;
