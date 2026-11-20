mod common;
use cognicloud_sdk::{
    Client, CreateStorageBucket, Error, Options, StorageDeleteOptions, StorageDownloadOptions,
    StorageObjectListOptions, StorageRange, StorageUploadOptions, StorageVisibility,
    UpdateStorageBucket,
};
use common::*;
use serde_json::{json, Value};
use wiremock::matchers::{
    body_bytes, body_json, header, method, path, query_param, query_param_is_missing,
};
use wiremock::{Mock, MockBuilder, MockServer, ResponseTemplate};

fn storage_client(server: &MockServer) -> Client {
    Client::builder()
        .with_keys()
        .storage_base_url(format!("{}/v1/storage", server.uri()))
        .max_retries(0)
        .build()
        .unwrap()
}

fn retrying_client(server: &MockServer) -> Client {
    Client::builder()
        .with_keys()
        .storage_base_url(format!("{}/v1/storage", server.uri()))
        .max_retries(2)
        .build()
        .unwrap()
}

/// A route on the storage service, answered only for the storage product key and the account key.
fn authed(verb: &str, route: &str) -> MockBuilder {
    Mock::given(method(verb))
        .and(path(format!("/v1/storage{route}")))
        .and(header("authorization", bearer(STORAGE)))
        .and(header(ACCOUNT_HEADER, ACCOUNT))
}

fn bucket_json(name: &str) -> Value {
    json!({
        "id": "4f1c2b6e-0000-4000-8000-000000000001",
        "name": name,
        "visibility": "private",
        "description": null,
        "objectCount": 2,
        "totalSize": 2048,
        "createdAt": "2026-11-20T10:00:00Z"
    })
}

fn object_json(key: &str) -> Value {
    json!({ "key": key, "size": 5, "etag": "\"abc\"", "lastModified": "2026-11-20T10:00:00Z" })
}

fn metadata_json(key: &str) -> Value {
    json!({
        "key": key,
        "size": 5,
        "contentType": "image/jpeg",
        "etag": "\"abc\"",
        "lastModified": "2026-11-20T10:00:00Z"
    })
}

#[tokio::test]
async fn lists_buckets_with_the_storage_and_account_keys() {
    let server = MockServer::start().await;
    authed("GET", "/buckets")
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({ "buckets": [bucket_json("photos")] })),
        )
        .expect(1)
        .mount(&server)
        .await;
    let buckets = storage_client(&server)
        .storage()
        .buckets()
        .list()
        .await
        .unwrap();
    assert_eq!(buckets.len(), 1);
    assert_eq!(buckets[0].name, "photos");
    assert_eq!(buckets[0].visibility, StorageVisibility::Private);
    assert_eq!(buckets[0].description, None);
    assert_eq!(buckets[0].total_size, 2048);
}

#[tokio::test]
async fn creates_a_bucket_with_only_the_fields_that_are_set() {
    let server = MockServer::start().await;
    authed("POST", "/buckets")
        .and(body_json(
            json!({ "name": "photos", "description": "Trip photos" }),
        ))
        .respond_with(ResponseTemplate::new(201).set_body_json(bucket_json("photos")))
        .expect(1)
        .mount(&server)
        .await;
    authed("POST", "/buckets")
        .and(body_json(json!({ "name": "docs" })))
        .respond_with(ResponseTemplate::new(201).set_body_json(bucket_json("docs")))
        .expect(1)
        .mount(&server)
        .await;
    let client = storage_client(&server);
    let photos = client
        .storage()
        .buckets()
        .create(CreateStorageBucket {
            name: "photos".to_owned(),
            description: Some("Trip photos".to_owned()),
        })
        .await
        .unwrap();
    assert_eq!(photos.name, "photos");
    let docs = client
        .storage()
        .buckets()
        .create(CreateStorageBucket {
            name: "docs".to_owned(),
            description: None,
        })
        .await
        .unwrap();
    assert_eq!(docs.name, "docs");
}

#[tokio::test]
async fn a_create_is_not_retried_after_a_503() {
    let server = MockServer::start().await;
    authed("POST", "/buckets")
        .respond_with(
            ResponseTemplate::new(503)
                .set_body_json(json!({ "error": "unavailable", "message": "later" })),
        )
        .expect(1)
        .mount(&server)
        .await;
    let error = retrying_client(&server)
        .storage()
        .buckets()
        .create(CreateStorageBucket {
            name: "photos".to_owned(),
            description: None,
        })
        .await
        .unwrap_err();
    assert_eq!(error.status(), 503);
}

#[tokio::test]
async fn gets_a_bucket_by_its_encoded_name() {
    let server = MockServer::start().await;
    authed("GET", "/buckets/my%20bucket")
        .respond_with(ResponseTemplate::new(200).set_body_json(bucket_json("my bucket")))
        .expect(1)
        .mount(&server)
        .await;
    let bucket = storage_client(&server)
        .storage()
        .buckets()
        .get("my bucket")
        .await
        .unwrap();
    assert_eq!(bucket.name, "my bucket");
}

#[tokio::test]
async fn updates_only_the_fields_that_are_set_and_clears_a_description() {
    let server = MockServer::start().await;
    authed("PATCH", "/buckets/photos")
        .and(body_json(json!({ "visibility": "public_read" })))
        .respond_with(ResponseTemplate::new(200).set_body_json(bucket_json("photos")))
        .expect(1)
        .mount(&server)
        .await;
    authed("PATCH", "/buckets/photos")
        .and(body_json(json!({ "description": null })))
        .respond_with(ResponseTemplate::new(200).set_body_json(bucket_json("photos")))
        .expect(1)
        .mount(&server)
        .await;
    let client = storage_client(&server);
    let buckets = client.storage().buckets();
    buckets
        .update(
            "photos",
            UpdateStorageBucket {
                visibility: Some(StorageVisibility::PublicRead),
                description: None,
            },
        )
        .await
        .unwrap();
    buckets
        .update(
            "photos",
            UpdateStorageBucket {
                visibility: None,
                description: Some(None),
            },
        )
        .await
        .unwrap();
}

#[tokio::test]
async fn deletes_a_bucket_without_force_and_with_force() {
    let server = MockServer::start().await;
    authed("DELETE", "/buckets/photos")
        .and(query_param_is_missing("force"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;
    authed("DELETE", "/buckets/photos")
        .and(query_param("force", "true"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;
    let client = storage_client(&server);
    let buckets = client.storage().buckets();
    buckets
        .delete("photos", StorageDeleteOptions::default())
        .await
        .unwrap();
    buckets
        .delete("photos", StorageDeleteOptions { force: true })
        .await
        .unwrap();
}

#[tokio::test]
async fn refuses_to_delete_a_non_empty_bucket_with_409() {
    let server = MockServer::start().await;
    authed("DELETE", "/buckets/docs")
        .respond_with(ResponseTemplate::new(409).set_body_json(json!({
            "error": "conflict",
            "message": "The bucket is not empty. Delete its objects first, or pass force=true.",
            "catalogCode": "cgco-1002"
        })))
        .expect(1)
        .mount(&server)
        .await;
    let error = storage_client(&server)
        .storage()
        .buckets()
        .delete("docs", StorageDeleteOptions::default())
        .await
        .unwrap_err();
    assert!(matches!(error, Error::Api { .. }));
    assert_eq!(error.status(), 409);
    assert_eq!(error.code(), "conflict");
    assert!(error.to_string().contains("not empty"));
}

#[tokio::test]
async fn lists_one_page_with_every_option() {
    let server = MockServer::start().await;
    authed("GET", "/buckets/photos/objects")
        .and(query_param("prefix", "2026/"))
        .and(query_param("delimiter", "/"))
        .and(query_param("cursor", "c1"))
        .and(query_param("limit", "50"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "folders": ["2026/raw/"],
            "objects": [object_json("2026/a.jpg")],
            "nextCursor": "c2"
        })))
        .expect(1)
        .mount(&server)
        .await;
    let page = storage_client(&server)
        .storage()
        .objects()
        .list(
            "photos",
            StorageObjectListOptions {
                prefix: Some("2026/".to_owned()),
                delimiter: Some("/".to_owned()),
                cursor: Some("c1".to_owned()),
                limit: Some(50),
            },
        )
        .await
        .unwrap();
    assert_eq!(page.folders, vec!["2026/raw/".to_owned()]);
    assert_eq!(page.objects[0].key, "2026/a.jpg");
    assert_eq!(page.next_cursor.as_deref(), Some("c2"));
}

#[tokio::test]
async fn lists_keys_flat_when_the_delimiter_is_empty() {
    let server = MockServer::start().await;
    authed("GET", "/buckets/photos/objects")
        .and(query_param("delimiter", ""))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "folders": [], "objects": [], "nextCursor": null
        })))
        .expect(1)
        .mount(&server)
        .await;
    storage_client(&server)
        .storage()
        .objects()
        .list(
            "photos",
            StorageObjectListOptions {
                delimiter: Some(String::new()),
                ..Default::default()
            },
        )
        .await
        .unwrap();
}

async fn mount_two_pages(server: &MockServer) {
    authed("GET", "/buckets/photos/objects")
        .and(query_param_is_missing("cursor"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "folders": ["a/"],
            "objects": [object_json("a/1.jpg"), object_json("a/2.jpg")],
            "nextCursor": "c2"
        })))
        .expect(1)
        .mount(server)
        .await;
    authed("GET", "/buckets/photos/objects")
        .and(query_param("cursor", "c2"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "folders": ["b/"],
            "objects": [object_json("b/3.jpg")],
            "nextCursor": null
        })))
        .expect(1)
        .mount(server)
        .await;
}

#[tokio::test]
async fn pages_yields_each_page_then_none() {
    let server = MockServer::start().await;
    mount_two_pages(&server).await;
    let client = storage_client(&server);
    let mut pages = client
        .storage()
        .objects()
        .pages("photos", StorageObjectListOptions::default());
    let first = pages.next().await.unwrap().unwrap();
    assert_eq!(first.objects.len(), 2);
    let second = pages.next().await.unwrap().unwrap();
    assert_eq!(second.objects[0].key, "b/3.jpg");
    assert!(pages.next().await.is_none());
    assert!(pages.next().await.is_none());
}

#[tokio::test]
async fn all_joins_every_page_into_one() {
    let server = MockServer::start().await;
    mount_two_pages(&server).await;
    let everything = storage_client(&server)
        .storage()
        .objects()
        .pages("photos", StorageObjectListOptions::default())
        .all()
        .await
        .unwrap();
    assert_eq!(everything.objects.len(), 3);
    assert_eq!(everything.folders, vec!["a/".to_owned(), "b/".to_owned()]);
    assert_eq!(everything.next_cursor, None);
}

#[tokio::test]
async fn a_failed_page_keeps_its_position_so_the_next_call_retries_it() {
    let server = MockServer::start().await;
    authed("GET", "/buckets/photos/objects")
        .and(query_param_is_missing("cursor"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "folders": [], "objects": [object_json("a.jpg")], "nextCursor": "c2"
        })))
        .expect(1)
        .mount(&server)
        .await;
    authed("GET", "/buckets/photos/objects")
        .and(query_param("cursor", "c2"))
        .respond_with(
            ResponseTemplate::new(500)
                .set_body_json(json!({ "error": "internal", "message": "oops" })),
        )
        .up_to_n_times(1)
        .mount(&server)
        .await;
    authed("GET", "/buckets/photos/objects")
        .and(query_param("cursor", "c2"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "folders": [], "objects": [object_json("b.jpg")], "nextCursor": null
        })))
        .expect(1)
        .mount(&server)
        .await;
    let client = storage_client(&server);
    let mut pages = client
        .storage()
        .objects()
        .pages("photos", StorageObjectListOptions::default());
    assert!(pages.next().await.unwrap().is_ok());
    assert_eq!(pages.next().await.unwrap().unwrap_err().status(), 500);
    let retried = pages.next().await.unwrap().unwrap();
    assert_eq!(retried.objects[0].key, "b.jpg");
    assert!(pages.next().await.is_none());
}

#[tokio::test]
async fn uploads_with_the_default_and_a_given_content_type() {
    let server = MockServer::start().await;
    authed("PUT", "/buckets/photos/objects/2026/a%20b.jpg")
        .and(header("content-type", "application/octet-stream"))
        .and(body_bytes(b"hello".to_vec()))
        .respond_with(ResponseTemplate::new(200).set_body_json(metadata_json("2026/a b.jpg")))
        .expect(1)
        .mount(&server)
        .await;
    authed("PUT", "/buckets/photos/objects/2026/a%20b.jpg")
        .and(header("content-type", "image/jpeg"))
        .and(body_bytes(b"jpeg".to_vec()))
        .respond_with(ResponseTemplate::new(200).set_body_json(metadata_json("2026/a b.jpg")))
        .expect(1)
        .mount(&server)
        .await;
    let client = storage_client(&server);
    let objects = client.storage().objects();
    let stored = objects
        .upload(
            "photos",
            "2026/a b.jpg",
            b"hello".to_vec(),
            StorageUploadOptions::default(),
        )
        .await
        .unwrap();
    assert_eq!(stored.content_type, "image/jpeg");
    objects
        .upload(
            "photos",
            "2026/a b.jpg",
            &b"jpeg"[..],
            StorageUploadOptions {
                content_type: Some("image/jpeg".to_owned()),
            },
        )
        .await
        .unwrap();
}

#[tokio::test]
async fn an_upload_is_not_retried_after_a_503() {
    let server = MockServer::start().await;
    authed("PUT", "/buckets/photos/objects/a.txt")
        .respond_with(
            ResponseTemplate::new(503)
                .set_body_json(json!({ "error": "unavailable", "message": "later" })),
        )
        .expect(1)
        .mount(&server)
        .await;
    let error = retrying_client(&server)
        .storage()
        .objects()
        .upload(
            "photos",
            "a.txt",
            b"x".to_vec(),
            StorageUploadOptions::default(),
        )
        .await
        .unwrap_err();
    assert_eq!(error.status(), 503);
}

#[tokio::test]
async fn downloads_the_whole_object_and_a_range() {
    let server = MockServer::start().await;
    authed("GET", "/buckets/photos/objects/whole.txt")
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"hello world".to_vec()))
        .expect(1)
        .mount(&server)
        .await;
    authed("GET", "/buckets/photos/objects/a.txt")
        .and(header("range", "bytes=0-4"))
        .respond_with(ResponseTemplate::new(206).set_body_bytes(b"hello".to_vec()))
        .expect(1)
        .mount(&server)
        .await;
    authed("GET", "/buckets/photos/objects/a.txt")
        .and(header("range", "bytes=6-"))
        .respond_with(ResponseTemplate::new(206).set_body_bytes(b"world".to_vec()))
        .expect(1)
        .mount(&server)
        .await;
    authed("GET", "/buckets/photos/objects/a.txt")
        .and(header("range", "bytes=-3"))
        .respond_with(ResponseTemplate::new(206).set_body_bytes(b"rld".to_vec()))
        .expect(1)
        .mount(&server)
        .await;
    let client = storage_client(&server);
    let objects = client.storage().objects();
    assert_eq!(
        objects
            .download("photos", "whole.txt", StorageDownloadOptions::default())
            .await
            .unwrap(),
        b"hello world"
    );
    for (range, expected) in [
        (StorageRange::Span(0, 4), &b"hello"[..]),
        (StorageRange::From(6), &b"world"[..]),
        (StorageRange::Last(3), &b"rld"[..]),
    ] {
        let bytes = objects
            .download(
                "photos",
                "a.txt",
                StorageDownloadOptions { range: Some(range) },
            )
            .await
            .unwrap();
        assert_eq!(bytes, expected);
    }
}

#[tokio::test]
async fn reads_metadata_with_an_encoded_key() {
    let server = MockServer::start().await;
    let key = "docs/zażółć + 1?.txt";
    authed(
        "GET",
        "/buckets/photos/objects/docs/za%C5%BC%C3%B3%C5%82%C4%87%20%2B%201%3F.txt",
    )
    .and(query_param("metadata", "true"))
    .respond_with(ResponseTemplate::new(200).set_body_json(metadata_json(key)))
    .expect(1)
    .mount(&server)
    .await;
    let meta = storage_client(&server)
        .storage()
        .objects()
        .metadata("photos", key)
        .await
        .unwrap();
    assert_eq!(meta.key, key);
    assert_eq!(meta.content_type, "image/jpeg");
    assert_eq!(meta.etag.as_deref(), Some("\"abc\""));
}

#[tokio::test]
async fn deletes_an_object() {
    let server = MockServer::start().await;
    authed("DELETE", "/buckets/photos/objects/2026/a.jpg")
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;
    storage_client(&server)
        .storage()
        .objects()
        .delete("photos", "2026/a.jpg")
        .await
        .unwrap();
}

#[tokio::test]
async fn creates_a_folder_with_its_prefix() {
    let server = MockServer::start().await;
    authed("POST", "/buckets/photos/folders")
        .and(body_json(json!({ "prefix": "2026/" })))
        .respond_with(ResponseTemplate::new(201))
        .expect(1)
        .mount(&server)
        .await;
    storage_client(&server)
        .storage()
        .folders()
        .create("photos", "2026/")
        .await
        .unwrap();
}

#[tokio::test]
async fn refuses_unsafe_object_keys_before_any_request() {
    let server = MockServer::start().await;
    Mock::given(wiremock::matchers::any())
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&server)
        .await;
    let client = storage_client(&server);
    let objects = client.storage().objects();
    for key in [
        "",
        "/a.txt",
        "a//b.txt",
        "a/./b.txt",
        "a/../b.txt",
        "a/..",
        "./a.txt",
        "a\nb.txt",
        "a\u{7f}.txt",
    ] {
        let errors = [
            objects
                .upload(
                    "photos",
                    key,
                    b"x".to_vec(),
                    StorageUploadOptions::default(),
                )
                .await
                .err(),
            objects
                .download("photos", key, StorageDownloadOptions::default())
                .await
                .err(),
            objects.metadata("photos", key).await.err(),
            objects.delete("photos", key).await.err(),
        ];
        for error in errors {
            let error = error.unwrap_or_else(|| panic!("{key:?} was not refused"));
            assert_eq!(error.code(), "invalid_key", "{key:?}");
        }
    }
}

#[tokio::test]
async fn refuses_unsafe_prefixes_and_folders_before_any_request() {
    let server = MockServer::start().await;
    Mock::given(wiremock::matchers::any())
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&server)
        .await;
    let client = storage_client(&server);
    let storage = client.storage();
    for prefix in ["/2026", "a//b", "a/../b", "a/./", "x\ty"] {
        let error = storage
            .objects()
            .list(
                "photos",
                StorageObjectListOptions {
                    prefix: Some(prefix.to_owned()),
                    ..Default::default()
                },
            )
            .await
            .unwrap_err();
        assert_eq!(error.code(), "invalid_key", "{prefix:?}");
    }
    for prefix in ["", "2026", "a/..", "/x/", "a//"] {
        let error = storage
            .folders()
            .create("photos", prefix)
            .await
            .unwrap_err();
        assert_eq!(error.code(), "invalid_key", "{prefix:?}");
    }
}

#[tokio::test]
async fn refuses_a_bucket_name_that_is_a_path_segment_trick() {
    let server = MockServer::start().await;
    Mock::given(wiremock::matchers::any())
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&server)
        .await;
    let client = storage_client(&server);
    let buckets = client.storage().buckets();
    for name in ["", "..", ".", "a/b"] {
        let error = buckets.get(name).await.unwrap_err();
        assert_eq!(error.code(), "invalid_key", "{name:?}");
    }
}

#[test]
fn the_storage_base_url_defaults_to_the_public_service() {
    assert_eq!(
        Options::default().storage_base_url,
        "https://cognicloud.dev/api/v1/storage"
    );
}
