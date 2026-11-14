use crate::error::{Error, Result};

/// What a key reaches: the whole workspace, or one product.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyKind {
    Account,
    Gateway,
    Database,
    Browser,
    Mail,
    Agents,
    Vault,
}

const PREFIXES: [(&str, KeyKind); 7] = [
    ("cc_acc_", KeyKind::Account),
    ("cc_gw_", KeyKind::Gateway),
    ("cc_db_", KeyKind::Database),
    ("cc_br_", KeyKind::Browser),
    ("cc_ml_", KeyKind::Mail),
    ("cc_ag_", KeyKind::Agents),
    ("cc_vt_", KeyKind::Vault),
];

const RETIRED: [&str; 7] = ["cc_live_", "ccag_", "cga_", "cgb_", "cgd_", "cgm_", "cgv_"];

const SETTINGS: &str =
    "Create an account key or a product key in the dashboard under Settings → API keys.";

/// A CogniCloud API key read without a request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedKey {
    pub kind: KeyKind,
    /// The workspace as the key and invoke URLs name it: its name as UTF-8, in lowercase hex.
    pub tenant_hex: String,
}

fn key_error(code: &'static str, message: String) -> Error {
    Error::Key { code, message }
}

/// Reads an account or product key, or fails with `missing_key`, `retired_key` or `invalid_key`.
pub fn parse_key(key: &str) -> Result<ParsedKey> {
    if key.trim().is_empty() {
        return Err(key_error(
            "missing_key",
            format!("An API key is required. {SETTINGS}"),
        ));
    }
    if let Some(old) = RETIRED.iter().find(|prefix| key.starts_with(*prefix)) {
        let message = format!("Keys starting with {old} were retired in SDK 1.0. {SETTINGS}");
        return Err(key_error("retired_key", message));
    }
    let invalid = || {
        key_error(
            "invalid_key",
            format!("This is not a CogniCloud API key. {SETTINGS}"),
        )
    };
    let (prefix, kind) = PREFIXES
        .iter()
        .find(|(prefix, _)| key.starts_with(prefix))
        .ok_or_else(invalid)?;
    let (tenant_hex, secret) = key[prefix.len()..].split_once('_').ok_or_else(invalid)?;
    let hex = !tenant_hex.is_empty()
        && tenant_hex.len() % 2 == 0
        && tenant_hex
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte));
    if !hex || secret.is_empty() || !secret.bytes().all(|byte| byte.is_ascii_alphanumeric()) {
        return Err(invalid());
    }
    Ok(ParsedKey {
        kind: *kind,
        tenant_hex: tenant_hex.to_owned(),
    })
}
