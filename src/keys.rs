use crate::error::{Error, Result};

/// What a key reaches: the whole workspace, or one product.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum KeyKind {
    Account,
    Gateway,
    Database,
    Browser,
    Mail,
    Agents,
    Vault,
    Storage,
}

const PREFIXES: [(&str, KeyKind); 8] = [
    ("cc_acc_", KeyKind::Account),
    ("cc_gw_", KeyKind::Gateway),
    ("cc_db_", KeyKind::Database),
    ("cc_br_", KeyKind::Browser),
    ("cc_ml_", KeyKind::Mail),
    ("cc_ag_", KeyKind::Agents),
    ("cc_vt_", KeyKind::Vault),
    ("cc_st_", KeyKind::Storage),
];

const RETIRED: [&str; 7] = ["cc_live_", "ccag_", "cga_", "cgb_", "cgd_", "cgm_", "cgv_"];

const PAIR_SETTINGS: &str = "Create your account key and a product key for each product in the dashboard under Settings → API keys.";
const SETTINGS: &str =
    "Create your account key and a product key for each product in the dashboard under Settings → API keys.";

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

/// The header that names the account key on every product call.
pub(crate) const ACCOUNT_HEADER: &str = "x-cognicloud-account-key";

pub(crate) fn product_label(kind: KeyKind) -> &'static str {
    match kind {
        KeyKind::Account => "Account",
        KeyKind::Gateway => "AI Gateway",
        KeyKind::Database => "Database",
        KeyKind::Browser => "Browser",
        KeyKind::Mail => "Mail",
        KeyKind::Agents => "Agents",
        KeyKind::Vault => "Vault",
        KeyKind::Storage => "Storage",
    }
}

fn prefix(kind: KeyKind) -> &'static str {
    PREFIXES
        .iter()
        .find(|(_, candidate)| *candidate == kind)
        .map_or("", |(prefix, _)| *prefix)
}

/// Reads the account key: `missing_account_key` when it is blank, `invalid_key` when it is another kind.
pub(crate) fn read_account(key: &str) -> Result<ParsedKey> {
    if key.trim().is_empty() {
        return Err(key_error(
            "missing_account_key",
            format!("An account key is required. {PAIR_SETTINGS}"),
        ));
    }
    let parsed = parse_key(key)?;
    if parsed.kind != KeyKind::Account {
        return Err(key_error(
            "invalid_key",
            format!("An account key (cc_acc_...) is needed here. {PAIR_SETTINGS}"),
        ));
    }
    Ok(parsed)
}

/// Reads a product key. An account key is `invalid_key`.
pub(crate) fn read_product(key: &str) -> Result<ParsedKey> {
    let parsed = parse_key(key)?;
    if parsed.kind == KeyKind::Account {
        return Err(key_error(
            "invalid_key",
            format!("This is an account key, but a product key is needed here. {PAIR_SETTINGS}"),
        ));
    }
    Ok(parsed)
}

/// Fails with `invalid_key` unless the key is the product key of `kind`.
pub(crate) fn require_kind(parsed: &ParsedKey, kind: KeyKind) -> Result<()> {
    if parsed.kind == kind {
        return Ok(());
    }
    Err(key_error(
        "invalid_key",
        format!(
            "The key must be the {} product key, starting with {}.",
            product_label(kind),
            prefix(kind)
        ),
    ))
}

/// Fails with `key_mismatch` unless both keys name the same workspace.
pub(crate) fn check_workspace(account: &ParsedKey, product: &ParsedKey) -> Result<()> {
    if account.tenant_hex == product.tenant_hex {
        return Ok(());
    }
    Err(key_error(
        "key_mismatch",
        "The account key and the product key must be from one workspace.".to_owned(),
    ))
}

fn article(kind: KeyKind) -> &'static str {
    match kind {
        KeyKind::Gateway => "an",
        _ => "a",
    }
}

pub(crate) fn missing_product(kind: KeyKind) -> Error {
    key_error(
        "missing_product_key",
        format!(
            "Add {} {} product key with .product_key(...), from Settings → API keys.",
            article(kind),
            product_label(kind)
        ),
    )
}
