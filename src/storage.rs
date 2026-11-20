use reqwest::Method;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::client::encode;
use crate::credentials::Auth;
use crate::error::{Error, Result};
use crate::http::{Call, Http, RawCall};

const MAX_KEY_BYTES: usize = 1024;
const DEFAULT_CONTENT_TYPE: &str = "application/octet-stream";

/// Who can read a bucket. `public_read` is stored only: the service serves nothing publicly yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StorageVisibility {
    Private,
    PublicRead,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageBucket {
    pub id: String,
    pub name: String,
    pub visibility: StorageVisibility,
    pub description: Option<String>,
    /// Approximate; updated after each write.
    pub object_count: u64,
    pub total_size: u64,
    pub created_at: String,
}

#[derive(Deserialize)]
struct BucketList {
    buckets: Vec<StorageBucket>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateStorageBucket {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// Only the fields that are `Some` are sent. `Some(None)` clears the description.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateStorageBucket {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub visibility: Option<StorageVisibility>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<Option<String>>,
}

/// `force` deletes a bucket's objects with it. Without it, a bucket that still holds objects is refused with `409`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct StorageDeleteOptions {
    pub force: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageObject {
    pub key: String,
    pub size: u64,
    pub etag: Option<String>,
    pub last_modified: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageObjectMetadata {
    pub key: String,
    pub size: u64,
    pub content_type: String,
    pub etag: Option<String>,
    pub last_modified: String,
}

/// Folders and objects under a prefix. Pass `next_cursor` as the cursor for the next page; it is `None` on the last.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageObjectPage {
    pub folders: Vec<String>,
    pub objects: Vec<StorageObject>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StorageObjectListOptions {
    /// Only keys under this prefix.
    pub prefix: Option<String>,
    /// Groups keys into folders. Default `/`; `Some("")` lists every key flat.
    pub delimiter: Option<String>,
    pub cursor: Option<String>,
    /// Entries per page, 1 to 1000. Default 100.
    pub limit: Option<u32>,
}

/// `content_type` defaults to `application/octet-stream`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StorageUploadOptions {
    pub content_type: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct StorageDownloadOptions {
    pub range: Option<StorageRange>,
}

/// A byte range of an object, sent as the `Range` header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageRange {
    /// From this byte to the end: `bytes=start-`.
    From(u64),
    /// From the first byte to the last, both included: `bytes=start-end`.
    Span(u64, u64),
    /// The last `n` bytes: `bytes=-n`.
    Last(u64),
}

impl StorageRange {
    fn header(self) -> String {
        match self {
            StorageRange::From(start) => format!("bytes={start}-"),
            StorageRange::Span(start, end) => format!("bytes={start}-{end}"),
            StorageRange::Last(count) => format!("bytes=-{count}"),
        }
    }
}

fn invalid_key(message: &str) -> Error {
    Error::Key {
        code: "invalid_key",
        message: message.to_owned(),
    }
}

fn segments_ok(value: &str) -> bool {
    if value.chars().any(char::is_control) {
        return false;
    }
    let parts: Vec<&str> = value.split('/').collect();
    let last = parts.len() - 1;
    parts.iter().enumerate().all(|(index, part)| {
        let empty = part.is_empty() && index != last;
        !empty && *part != "." && *part != ".."
    })
}

/// An object key: 1 to 1024 bytes, no control characters, no leading slash, no empty segment except a trailing one,
/// and no `.` or `..` segment. Refused before any request, so no URL normalisation can reach another object.
fn check_key(key: &str) -> Result<()> {
    if key.is_empty() || key.len() > MAX_KEY_BYTES || !segments_ok(key) {
        return Err(invalid_key(
            "An object key must be 1 to 1024 bytes, without control characters, a leading slash, an empty segment or . and .. segments.",
        ));
    }
    Ok(())
}

/// A listing prefix: may be empty, and may end mid-name or with a slash.
fn check_prefix(prefix: &str) -> Result<()> {
    if prefix.len() > MAX_KEY_BYTES || !segments_ok(prefix) {
        return Err(invalid_key(
            "A prefix must be at most 1024 bytes, without control characters, a leading slash, an empty segment or . and .. segments.",
        ));
    }
    Ok(())
}

/// A folder key: a valid key that ends with a slash.
fn check_folder(prefix: &str) -> Result<()> {
    if !prefix.ends_with('/') {
        return Err(invalid_key("A folder key must end with a slash."));
    }
    check_key(prefix)
}

/// A bucket name as a path segment: not empty, no slash, no control characters, not `.` or `..`.
fn check_bucket(bucket: &str) -> Result<()> {
    if bucket.is_empty()
        || bucket.contains('/')
        || bucket == "."
        || bucket == ".."
        || bucket.chars().any(char::is_control)
    {
        return Err(invalid_key(
            "A bucket name must not be empty, contain a slash or control characters, or be . or ..",
        ));
    }
    Ok(())
}

/// Each segment of a key is encoded on its own, so the slashes between segments stay.
fn key_path(key: &str) -> String {
    key.split('/').map(encode).collect::<Vec<_>>().join("/")
}

fn object_path(bucket: &str, key: &str) -> String {
    format!("/buckets/{}/objects/{}", encode(bucket), key_path(key))
}

fn new_call(auth: Auth<'_>, method: Method, path: String, retry: bool) -> Result<Call<'static>> {
    Ok(Call {
        method,
        path,
        headers: auth.headers()?,
        query: Vec::new(),
        body: None,
        retry,
    })
}

fn body_of<T: Serialize>(value: &T) -> Result<Value> {
    serde_json::to_value(value).map_err(|error| Error::Config(error.to_string()))
}

/// Buckets, objects and folders, from [`Client::storage`](crate::Client::storage). Needs the Storage product key.
pub struct StorageApi<'a> {
    http: &'a Http,
    auth: Auth<'a>,
}

impl<'a> StorageApi<'a> {
    pub(crate) fn new(http: &'a Http, auth: Auth<'a>) -> Self {
        Self { http, auth }
    }

    pub fn buckets(&self) -> StorageBucketsApi<'a> {
        StorageBucketsApi {
            http: self.http,
            auth: self.auth,
        }
    }

    pub fn objects(&self) -> StorageObjectsApi<'a> {
        StorageObjectsApi {
            http: self.http,
            auth: self.auth,
        }
    }

    pub fn folders(&self) -> StorageFoldersApi<'a> {
        StorageFoldersApi {
            http: self.http,
            auth: self.auth,
        }
    }
}

/// Creates, lists, reads, changes and deletes buckets.
#[derive(Clone, Copy)]
pub struct StorageBucketsApi<'a> {
    http: &'a Http,
    auth: Auth<'a>,
}

impl StorageBucketsApi<'_> {
    pub async fn list(&self) -> Result<Vec<StorageBucket>> {
        let list: BucketList = self
            .http
            .send(new_call(
                self.auth,
                Method::GET,
                "/buckets".to_owned(),
                true,
            )?)
            .await?;
        Ok(list.buckets)
    }

    pub async fn create(&self, input: CreateStorageBucket) -> Result<StorageBucket> {
        let mut call = new_call(self.auth, Method::POST, "/buckets".to_owned(), false)?;
        call.body = Some(body_of(&input)?);
        self.http.send(call).await
    }

    pub async fn get(&self, name: &str) -> Result<StorageBucket> {
        check_bucket(name)?;
        let path = format!("/buckets/{}", encode(name));
        self.http
            .send(new_call(self.auth, Method::GET, path, true)?)
            .await
    }

    pub async fn update(&self, name: &str, patch: UpdateStorageBucket) -> Result<StorageBucket> {
        check_bucket(name)?;
        let path = format!("/buckets/{}", encode(name));
        let mut call = new_call(self.auth, Method::PATCH, path, false)?;
        call.body = Some(body_of(&patch)?);
        self.http.send(call).await
    }

    /// Refused with `409` (`conflict`) while the bucket holds objects, unless `options.force` is set.
    pub async fn delete(&self, name: &str, options: StorageDeleteOptions) -> Result<()> {
        check_bucket(name)?;
        let path = format!("/buckets/{}", encode(name));
        let mut call = new_call(self.auth, Method::DELETE, path, false)?;
        if options.force {
            call.query.push(("force", "true".to_owned()));
        }
        self.http.send(call).await
    }
}

/// Uploads, downloads, lists and deletes objects.
#[derive(Clone, Copy)]
pub struct StorageObjectsApi<'a> {
    http: &'a Http,
    auth: Auth<'a>,
}

impl<'a> StorageObjectsApi<'a> {
    /// One page of folders and objects. Pass the returned `next_cursor` back to continue.
    pub async fn list(
        &self,
        bucket: &str,
        options: StorageObjectListOptions,
    ) -> Result<StorageObjectPage> {
        check_bucket(bucket)?;
        if let Some(prefix) = &options.prefix {
            check_prefix(prefix)?;
        }
        let path = format!("/buckets/{}/objects", encode(bucket));
        let mut call = new_call(self.auth, Method::GET, path, true)?;
        if let Some(prefix) = options.prefix {
            call.query.push(("prefix", prefix));
        }
        if let Some(delimiter) = options.delimiter {
            call.query.push(("delimiter", delimiter));
        }
        if let Some(cursor) = options.cursor {
            call.query.push(("cursor", cursor));
        }
        if let Some(limit) = options.limit {
            call.query.push(("limit", limit.to_string()));
        }
        self.http.send(call).await
    }

    /// Every page of a listing, one request at a time. See [`StorageObjectPages::next`].
    pub fn pages(&self, bucket: &str, options: StorageObjectListOptions) -> StorageObjectPages<'a> {
        StorageObjectPages {
            objects: *self,
            bucket: bucket.to_owned(),
            options,
            finished: false,
        }
    }

    /// Stores the bytes under `key`, replacing any object there. Up to 100 MB. Sent once, never retried.
    pub async fn upload(
        &self,
        bucket: &str,
        key: &str,
        body: impl Into<Vec<u8>>,
        options: StorageUploadOptions,
    ) -> Result<StorageObjectMetadata> {
        check_bucket(bucket)?;
        check_key(key)?;
        self.http
            .send_raw(RawCall {
                method: Method::PUT,
                path: object_path(bucket, key),
                headers: self.auth.headers()?,
                query: Vec::new(),
                body: body.into(),
                content_type: options
                    .content_type
                    .unwrap_or_else(|| DEFAULT_CONTENT_TYPE.to_owned()),
            })
            .await
    }

    /// The object's bytes, or only the requested range of them.
    pub async fn download(
        &self,
        bucket: &str,
        key: &str,
        options: StorageDownloadOptions,
    ) -> Result<Vec<u8>> {
        check_bucket(bucket)?;
        check_key(key)?;
        let mut call = new_call(self.auth, Method::GET, object_path(bucket, key), true)?;
        if let Some(range) = options.range {
            call.headers.push(("range", range.header()));
        }
        self.http.send_bytes(call).await
    }

    /// Size, content type, etag and last modified time, without the bytes.
    pub async fn metadata(&self, bucket: &str, key: &str) -> Result<StorageObjectMetadata> {
        check_bucket(bucket)?;
        check_key(key)?;
        let mut call = new_call(self.auth, Method::GET, object_path(bucket, key), true)?;
        call.query.push(("metadata", "true".to_owned()));
        self.http.send(call).await
    }

    pub async fn delete(&self, bucket: &str, key: &str) -> Result<()> {
        check_bucket(bucket)?;
        check_key(key)?;
        self.http
            .send(new_call(
                self.auth,
                Method::DELETE,
                object_path(bucket, key),
                false,
            )?)
            .await
    }
}

/// The pages of one listing. Each [`next`](StorageObjectPages::next) call is one request.
pub struct StorageObjectPages<'a> {
    objects: StorageObjectsApi<'a>,
    bucket: String,
    options: StorageObjectListOptions,
    finished: bool,
}

impl StorageObjectPages<'_> {
    /// The next page, or `None` after the last one. A failed page keeps its position, so calling again retries it.
    pub async fn next(&mut self) -> Option<Result<StorageObjectPage>> {
        if self.finished {
            return None;
        }
        match self.objects.list(&self.bucket, self.options.clone()).await {
            Ok(page) => {
                match page.next_cursor.as_deref() {
                    Some(cursor) if !cursor.is_empty() => {
                        self.options.cursor = Some(cursor.to_owned());
                    }
                    _ => self.finished = true,
                }
                Some(Ok(page))
            }
            Err(error) => Some(Err(error)),
        }
    }

    /// Every remaining page joined: all folders and objects, with no `next_cursor`.
    pub async fn all(mut self) -> Result<StorageObjectPage> {
        let mut joined = StorageObjectPage {
            folders: Vec::new(),
            objects: Vec::new(),
            next_cursor: None,
        };
        while let Some(page) = self.next().await {
            let page = page?;
            joined.folders.extend(page.folders);
            joined.objects.extend(page.objects);
        }
        Ok(joined)
    }
}

/// Folders are marker objects, so an empty folder stays listed.
#[derive(Clone, Copy)]
pub struct StorageFoldersApi<'a> {
    http: &'a Http,
    auth: Auth<'a>,
}

impl StorageFoldersApi<'_> {
    /// Creates the folder `prefix`, which must end with a slash. Creating one that already exists is fine.
    pub async fn create(&self, bucket: &str, prefix: &str) -> Result<()> {
        check_bucket(bucket)?;
        check_folder(prefix)?;
        let mut call = new_call(
            self.auth,
            Method::POST,
            format!("/buckets/{}/folders", encode(bucket)),
            false,
        )?;
        call.body = Some(json!({ "prefix": prefix }));
        self.http.send(call).await
    }
}
