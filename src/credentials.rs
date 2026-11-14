use std::fmt;

use crate::error::Result;
use crate::keys::{
    check_workspace, missing_product, read_account, read_product, KeyKind, ACCOUNT_HEADER,
};

/// The keys of a client: the account key and one product key per product, all from one workspace.
pub(crate) struct Credentials {
    account: String,
    workspace: String,
    products: Vec<(KeyKind, String)>,
}

impl Credentials {
    /// Checks the account key and each product key. A second key for the same product replaces the first.
    pub(crate) fn resolve<'k>(
        account: &str,
        products: impl IntoIterator<Item = &'k str>,
    ) -> Result<Self> {
        let account_key = read_account(account)?;
        let mut kept: Vec<(KeyKind, String)> = Vec::new();
        for key in products {
            let product_key = read_product(key)?;
            check_workspace(&account_key, &product_key)?;
            match kept.iter().position(|(kind, _)| *kind == product_key.kind) {
                Some(index) => kept[index].1 = key.to_owned(),
                None => kept.push((product_key.kind, key.to_owned())),
            }
        }
        Ok(Self {
            account: account.to_owned(),
            workspace: account_key.tenant_hex,
            products: kept,
        })
    }

    pub(crate) fn account(&self) -> &str {
        &self.account
    }

    /// The workspace as the key names it: its name as UTF-8, in lowercase hex.
    pub(crate) fn workspace(&self) -> &str {
        &self.workspace
    }

    pub(crate) fn product(&self, kind: KeyKind) -> Result<&str> {
        self.products
            .iter()
            .find(|(held, _)| *held == kind)
            .map(|(_, key)| key.as_str())
            .ok_or_else(|| missing_product(kind))
    }

    /// `None` sends the account key alone, as the web builder and marketplace do.
    pub(crate) fn auth(&self, product: Option<KeyKind>) -> Auth<'_> {
        Auth {
            credentials: self,
            product,
        }
    }
}

impl fmt::Debug for Credentials {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let products: Vec<(KeyKind, &str)> = self
            .products
            .iter()
            .map(|(kind, _)| (*kind, "Secret(****)"))
            .collect();
        f.debug_struct("Credentials")
            .field("account_key", &"Secret(****)")
            .field("product_keys", &products)
            .finish()
    }
}

/// The authorization headers of one call.
#[derive(Clone, Copy)]
pub(crate) struct Auth<'a> {
    credentials: &'a Credentials,
    product: Option<KeyKind>,
}

impl Auth<'_> {
    /// A product call sends its product key as the Bearer token and the account key in its own header.
    pub(crate) fn headers(self) -> Result<Vec<(&'static str, String)>> {
        match self.product {
            Some(kind) => Ok(vec![
                (
                    "authorization",
                    format!("Bearer {}", self.credentials.product(kind)?),
                ),
                (ACCOUNT_HEADER, self.credentials.account().to_owned()),
            ]),
            None => Ok(vec![(
                "authorization",
                format!("Bearer {}", self.credentials.account()),
            )]),
        }
    }
}
