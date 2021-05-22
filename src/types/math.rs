use super::primitives::{u64_to_fr, Fr};
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;

pub fn number_to_integer(num: &Decimal, prec: u32) -> Fr {
    let prec_mul = Decimal::new(10, 0).powi(prec as u64);
    let adjusted = num * prec_mul;
    u64_to_fr(adjusted.floor().to_u64().unwrap())
}

#[cfg(test)]
#[test]
fn test_number_to_integer() {
    let pi = Decimal::new(3141, 3);
    let out = number_to_integer(&pi, 3);
    assert_eq!(
        "Fr(0x0000000000000000000000000000000000000000000000000000000000000c45)",
        out.to_string()
    );
}
