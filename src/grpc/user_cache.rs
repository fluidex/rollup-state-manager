use fluidex_common::db::models::account::AccountDesc;
use fluidex_common::fnv::FnvHashMap;

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

    pub fn set_user_info(&mut self, user_id: u32, l1_address: &str, l2_pubkey: &str) {
        let user_info = AccountDesc {
            id: user_id as i32,
            l1_address: l1_address.to_lowercase(),
            l2_pubkey: l2_pubkey.to_lowercase(),
        };

        self.users.insert(format_user_id_key(user_id), user_info.clone());
        self.users.insert(format_l1_address_key(l1_address), user_info.clone());
        self.users.insert(format_l2_pubkey_key(l2_pubkey), user_info);
    }
}

fn format_user_id_key(val: u32) -> String {
    format!("id:{}", val)
}

fn format_l1_address_key(val: &str) -> String {
    format!("l1_addr:{}", val.to_lowercase())
}

fn format_l2_pubkey_key(val: &str) -> String {
    format!("l2_pubkey:{}", val.to_lowercase())
}
