// TODO: Moves other test types to here.

// TODO: enum & impl
pub fn get_token_id_by_name(token_name: &str) -> u32 {
    match token_name {
        "ETH" => 0,
        "USDT" => 1,
        _ => unreachable!(),
    }
}

// TODO: enum & impl
pub fn prec_token_id(token_id: u32) -> u32 {
    match token_id {
        0 | 1 => 6,
        _ => unreachable!(),
    }
}
