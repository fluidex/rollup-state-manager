use super::primitives::{bigint_to_fr, u64_to_fr, Fr};
use num_traits::pow::Pow;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use std::convert::TryInto;

use anyhow::bail;
use anyhow::Result;
use num_bigint::BigInt;

pub fn decimal_to_u64(num: &Decimal, prec: u32) -> u64 {
    let prec_mul = Decimal::new(10, 0).pow(prec as u64);
    let adjusted = num * prec_mul;
    adjusted.floor().to_u64().unwrap()
}

pub fn decimal_to_fr(num: &Decimal, prec: u32) -> Fr {
    // TODO: is u64 enough?
    u64_to_fr(decimal_to_u64(num, prec))
    // Float864::from_decimal(num, prec).unwrap().to_fr()
}

pub fn decimal_to_amount(num: &Decimal, prec: u32) -> Float864 {
    Float864::from_decimal(num, prec).unwrap()
}

#[cfg(test)]
#[test]
fn test_decimal_to_fr() {
    let pi = Decimal::new(3141, 3);
    let out = decimal_to_fr(&pi, 3);
    assert_eq!(
        "Fr(0x0000000000000000000000000000000000000000000000000000000000000c45)",
        out.to_string()
    );
}

#[derive(Debug, Clone, Copy)]
pub struct Float864 {
    pub exponent: u8,
    // 5 bytes seems enough?
    pub significand: u64,
}

impl Float864 {
    pub fn to_bigint(&self) -> BigInt {
        let s = BigInt::from(self.significand);
        s * BigInt::from(10).pow(self.exponent)
    }
    pub fn to_fr(&self) -> Fr {
        bigint_to_fr(self.to_bigint())
    }
    pub fn encode(&self) -> Vec<u8> {
        let mut result = self.exponent.to_be_bytes().to_vec();
        result.append(&mut self.significand.to_be_bytes().to_vec());
        result
    }
    pub fn decode(data: &[u8]) -> Result<Self> {
        let exponent = u8::from_be_bytes(data[0..1].try_into()?);
        let significand = u64::from_be_bytes(data[1..9].try_into()?);
        Ok(Self { exponent, significand })
    }
    pub fn to_decimal(&self, prec: u32) -> Decimal {
        // for example, (significand:1, exponent:17) means 10**17, when prec is 18,
        // it is 0.1 (ETH)
        Decimal::new(self.significand as i64, 0) * Decimal::new(10, 0).pow(self.exponent as u64) / Decimal::new(10, 0).pow(prec as u64)
    }
    pub fn from_decimal(d: &Decimal, prec: u32) -> Result<Self> {
        // if d is "0.1" and prec is 18, result is (significand:1, exponent:17)
        if d.is_zero() {
            return Ok(Self {
                exponent: 0,
                significand: 0,
            });
        }
        let ten = Decimal::new(10, 0);
        let exp = ten.pow(prec as u64);
        let mut n = d * exp;
        if n != n.floor() {
            bail!("decimal precision error {} {}", d, prec);
        }
        let mut exponent = 0;
        loop {
            let next = n / ten;
            if next == next.floor() {
                exponent += 1;
                n = next;
            } else {
                break;
            }
        }
        if n > Decimal::new((std::u64::MAX / 4) as i64, 0) {
            bail!("invalid precision {} {} {}", d, prec, n);
        }
        // TODO: a better way...
        let significand: u64 = n.floor().to_string().parse::<u64>()?;
        Ok(Float864 { exponent, significand })
    }
}

#[cfg(test)]
#[test]
fn test_float864() {
    use std::str::FromStr;
    // 1.23456 * 10**18
    let d0 = Decimal::new(123456, 5);
    let f = Float864::from_decimal(&d0, 18).unwrap();
    assert_eq!(f.exponent, 13);
    assert_eq!(f.significand, 123456);
    let d = f.to_decimal(18);
    assert_eq!(d, Decimal::from_str("1.23456").unwrap());
    let f2 = Float864::decode(&f.encode()).unwrap();
    assert_eq!(f2.exponent, 13);
    assert_eq!(f2.significand, 123456);
}
