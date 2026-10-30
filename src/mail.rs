use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::http::{Call, Http};

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Mailbox {
    pub id: String,
    pub address: String,
    pub local_part: String,
    pub domain: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MailboxList {
    pub items: Vec<Mailbox>,
    pub total: u64,
    pub limit: u32,
    pub offset: u32,
}

/// `password` is returned once; the server keeps no copy.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct CreatedMailbox {
    pub mailbox: Mailbox,
    pub password: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct RotatedMailboxPassword {
    pub password: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct MailServerEndpoint {
    pub host: String,
    pub port: u16,
    pub security: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MailConnectionSettings {
    /// The shared domain mailboxes are on unless a custom domain is chosen.
    #[serde(default)]
    pub default_domain: String,
    pub username_format: String,
    pub imap: MailServerEndpoint,
    pub smtp: MailServerEndpoint,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MailboxListOptions {
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CreateMailbox<'a> {
    local_part: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    domain: Option<&'a str>,
}

/// One DNS record to publish for a domain. `ok` is the last check's result, `None` when not checked.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct MailDnsRecord {
    #[serde(rename = "type")]
    pub kind: String,
    pub host: String,
    /// Exactly what to enter at the DNS provider; long TXT values are already split into quoted strings.
    pub value: String,
    /// `ownership`, `mx`, `spf`, `dkim` or `dmarc`.
    pub purpose: String,
    pub ok: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MailDomain {
    pub id: String,
    pub name: String,
    /// `pending`, `verified` or `needs_attention`.
    pub status: String,
    pub records: Vec<MailDnsRecord>,
    /// Mail servers the domain uses today besides CogniCloud.
    #[serde(default)]
    pub other_mx: Vec<String>,
    pub created_at: String,
    pub verified_at: Option<String>,
    pub last_checked_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct MailDomainList {
    pub items: Vec<MailDomain>,
}

#[derive(Serialize)]
struct AddDomain<'a> {
    name: &'a str,
}

/// Creates, lists and removes mailboxes on the Flippico mail server.
pub struct MailApi<'a> {
    http: &'a Http,
    headers: Vec<(&'static str, String)>,
}

impl<'a> MailApi<'a> {
    pub(crate) fn new(http: &'a Http, token: &str, tenant: &str) -> Self {
        let headers = vec![
            ("authorization", format!("Bearer {token}")),
            ("tenant_id", tenant.to_owned()),
        ];
        Self { http, headers }
    }

    /// Newest first.
    pub async fn list(&self, options: MailboxListOptions) -> Result<MailboxList> {
        let mut query = Vec::new();
        if let Some(limit) = options.limit {
            query.push(("limit", limit.to_string()));
        }
        if let Some(offset) = options.offset {
            query.push(("offset", offset.to_string()));
        }
        self.http
            .send(Call {
                method: reqwest::Method::GET,
                path: "/mailboxes".to_owned(),
                headers: self.headers.clone(),
                query,
                body: None,
                retry: true,
            })
            .await
    }

    /// Creates `<local_part>@<domain>` on the shared domain. Save the returned password now; it is not stored.
    pub async fn create(&self, local_part: &str) -> Result<CreatedMailbox> {
        self.create_mailbox(local_part, None).await
    }

    /// Creates `<local_part>@<domain>` on one of your verified domains. Save the returned password now.
    pub async fn create_on(&self, local_part: &str, domain: &str) -> Result<CreatedMailbox> {
        self.create_mailbox(local_part, Some(domain)).await
    }

    async fn create_mailbox(
        &self,
        local_part: &str,
        domain: Option<&str>,
    ) -> Result<CreatedMailbox> {
        self.http
            .send(Call {
                method: reqwest::Method::POST,
                path: "/mailboxes".to_owned(),
                headers: self.headers.clone(),
                query: Vec::new(),
                body: Some(
                    serde_json::to_value(CreateMailbox { local_part, domain }).unwrap_or_default(),
                ),
                retry: false,
            })
            .await
    }

    /// Your own domains for mailboxes.
    pub fn domains(&self) -> MailDomainsApi<'_> {
        MailDomainsApi {
            http: self.http,
            headers: self.headers.clone(),
        }
    }

    pub async fn get(&self, id: &str) -> Result<Mailbox> {
        self.http
            .send(Call {
                method: reqwest::Method::GET,
                path: format!("/mailboxes/{}", encode(id)),
                headers: self.headers.clone(),
                query: Vec::new(),
                body: None,
                retry: true,
            })
            .await
    }

    /// Deletes the mailbox and its mail on the server.
    pub async fn delete(&self, id: &str) -> Result<()> {
        self.http
            .send(Call {
                method: reqwest::Method::DELETE,
                path: format!("/mailboxes/{}", encode(id)),
                headers: self.headers.clone(),
                query: Vec::new(),
                body: None,
                retry: false,
            })
            .await
    }

    /// Replaces the password on the server and returns the new one once.
    pub async fn rotate_password(&self, id: &str) -> Result<RotatedMailboxPassword> {
        self.http
            .send(Call {
                method: reqwest::Method::POST,
                path: format!("/mailboxes/{}/password/rotate", encode(id)),
                headers: self.headers.clone(),
                query: Vec::new(),
                body: None,
                retry: false,
            })
            .await
    }

    /// Where mail clients connect with IMAP and SMTP.
    pub async fn connection_settings(&self) -> Result<MailConnectionSettings> {
        self.http
            .send(Call {
                method: reqwest::Method::GET,
                path: "/mailboxes/settings".to_owned(),
                headers: self.headers.clone(),
                query: Vec::new(),
                body: None,
                retry: true,
            })
            .await
    }
}

/// Your own domains for mailboxes. Adding, verifying and deleting need a signed-in person's token; an API key can
/// list them and create mailboxes on verified ones.
pub struct MailDomainsApi<'a> {
    http: &'a Http,
    headers: Vec<(&'static str, String)>,
}

impl MailDomainsApi<'_> {
    async fn call<T: serde::de::DeserializeOwned>(
        &self,
        method: reqwest::Method,
        path: String,
        body: Option<serde_json::Value>,
        retry: bool,
    ) -> Result<T> {
        self.http
            .send(Call {
                method,
                path,
                headers: self.headers.clone(),
                query: Vec::new(),
                body,
                retry,
            })
            .await
    }

    /// Newest first, each with the DNS records to publish.
    pub async fn list(&self) -> Result<MailDomainList> {
        self.call(reqwest::Method::GET, "/domains".to_owned(), None, true)
            .await
    }

    pub async fn get(&self, id: &str) -> Result<MailDomain> {
        self.call(
            reqwest::Method::GET,
            format!("/domains/{}", encode(id)),
            None,
            true,
        )
        .await
    }

    /// Adds the domain as pending. Publish its records, then call `verify`.
    pub async fn add(&self, name: &str) -> Result<MailDomain> {
        let body = serde_json::to_value(AddDomain { name }).unwrap_or_default();
        self.call(
            reqwest::Method::POST,
            "/domains".to_owned(),
            Some(body),
            false,
        )
        .await
    }

    /// Checks the domain's DNS now; the first time the ownership record is found, it becomes verified.
    pub async fn verify(&self, id: &str) -> Result<MailDomain> {
        self.call(
            reqwest::Method::POST,
            format!("/domains/{}/verify", encode(id)),
            None,
            false,
        )
        .await
    }

    /// Refused while the domain has mailboxes.
    pub async fn delete(&self, id: &str) -> Result<()> {
        self.call(
            reqwest::Method::DELETE,
            format!("/domains/{}", encode(id)),
            None,
            false,
        )
        .await
    }
}

fn encode(id: &str) -> String {
    id.bytes()
        .flat_map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                vec![byte as char]
            }
            _ => format!("%{byte:02X}").chars().collect(),
        })
        .collect()
}
