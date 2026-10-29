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

    /// Creates `<local_part>@<domain>`. Save the returned password now; it is not stored.
    pub async fn create(&self, local_part: &str) -> Result<CreatedMailbox> {
        self.http
            .send(Call {
                method: reqwest::Method::POST,
                path: "/mailboxes".to_owned(),
                headers: self.headers.clone(),
                query: Vec::new(),
                body: Some(serde_json::to_value(CreateMailbox { local_part }).unwrap_or_default()),
                retry: false,
            })
            .await
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

fn encode(id: &str) -> String {
    id.bytes()
        .flat_map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => vec![byte as char],
            _ => format!("%{byte:02X}").chars().collect(),
        })
        .collect()
}
