#![allow(dead_code)]

use cognicloud_sdk::ClientBuilder;

/// The workspace every test key belongs to: `cognicloud`, in hex.
pub const ACCOUNT: &str =
    "cc_acc_636f676e69636c6f7564_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
pub const GATEWAY: &str =
    "cc_gw_636f676e69636c6f7564_eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee";
pub const BROWSER: &str =
    "cc_br_636f676e69636c6f7564_bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
pub const MAIL: &str =
    "cc_ml_636f676e69636c6f7564_mmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmm";
pub const DATABASE: &str =
    "cc_db_636f676e69636c6f7564_dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd";
pub const AGENTS: &str =
    "cc_ag_636f676e69636c6f7564_cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";
pub const STORAGE: &str =
    "cc_st_636f676e69636c6f7564_ssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssss";

pub const ACCOUNT_HEADER: &str = "x-cognicloud-account-key";

pub fn bearer(key: &str) -> String {
    format!("Bearer {key}")
}

/// Adds the account key and one product key for every product, all from the test workspace.
pub trait WithKeys {
    fn with_keys(self) -> ClientBuilder;
}

impl WithKeys for ClientBuilder {
    fn with_keys(self) -> ClientBuilder {
        self.account_key(ACCOUNT)
            .product_key(GATEWAY)
            .product_key(BROWSER)
            .product_key(MAIL)
            .product_key(DATABASE)
            .product_key(AGENTS)
            .product_key(STORAGE)
    }
}
