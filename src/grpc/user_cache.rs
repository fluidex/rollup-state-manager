use fluidex_common::db::models::account::AccountDesc;
use fluidex_common::fnv::FnvHashMap;
use std::fmt::Display;

pub struct UserCache {
    // `id:{}`, `l1_addr:{}` or `l2_pubkey:{}` -> AccountDesc
    users: FnvHashMap<String, AccountDesc>,
}

impl UserCache {
    pub fn new() -> Self {
        Self {
            users: FnvHashMap::default(),
        }
    }

    pub fn get_user_info(&self, user_id: Option<u32>, l1_address: &Option<String>, l2_pubkey: &Option<String>) -> Option<&AccountDesc> {
        user_id
            .and_then(|val| self.users.get(&format_user_id_key(val)))
            .or_else(|| l1_address.as_ref().and_then(|val| self.users.get(&format_l1_address_key(val))))
            .or_else(|| l2_pubkey.as_ref().and_then(|val| self.users.get(&format_l2_pubkey_key(val))))
    }

    pub fn set_user_info(&mut self, user_info: AccountDesc) {
        self.users.insert(format_user_id_key(&user_info.id), user_info.clone());
        self.users.insert(format_l1_address_key(&user_info.l1_address), user_info.clone());
        self.users.insert(format_l2_pubkey_key(&user_info.l2_pubkey), user_info);
    }
}

fn format_user_id_key<T: Display>(val: T) -> String {
    format!("id:{}", val)
}

fn format_l1_address_key<T: Display>(val: T) -> String {
    format!("l1_addr:{}", val)
}

fn format_l2_pubkey_key<T: Display>(val: T) -> String {
    format!("l2_pubkey:{}", val)
}
