//! Integration tests for the CID Router server.
//!
//! Azure tests require the following environment variables:
//! - AZURE_STORAGE_ACCOUNT: The Azure storage account name
//! - AZURE_STORAGE_ACCESS_KEY: The Azure storage access key
//!
//! Run with: RUST_LOG=info cargo test -p cid-router-server --test integration -- --nocapture

use std::sync::Arc;

use cid_router_server::{
    config::{Config, ProviderConfig},
    context::Context,
};
use crp_azure::{ContainerConfig, config::ContainerBlobFilter};
use log::info;
use reqwest::Client;
use tokio::net::TcpListener;

/// Initialize logging. Safe to call multiple times (for parallel tests).
fn init_logging() {
    let _ = env_logger::try_init();
}

/// Find an available port by binding to port 0
async fn find_available_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    listener.local_addr().unwrap().port()
}

async fn start_test_server_azure(port: u16) -> tokio::task::JoinHandle<()> {
    let account = std::env::var("AZURE_STORAGE_ACCOUNT")
        .expect("Set AZURE_STORAGE_ACCOUNT env var for this test");

    let config = Config {
        port,
        auth: cid_router_server::auth::Auth::None,
        providers: vec![ProviderConfig::Azure(ContainerConfig {
            account,
            container: "blobs".to_string(),
            credentials: None, // Uses AZURE_STORAGE_ACCESS_KEY env var
            filter: ContainerBlobFilter::All,
            writeable: true,
        })],
    };

    let ctx = Context::init_in_memory(config)
        .await
        .expect("Failed to init context");

    let handle = tokio::spawn(async move {
        cid_router_server::api::start(Arc::new(ctx))
            .await
            .expect("Server failed");
    });

    // Give the server time to start
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    handle
}

/// Helper to post data and return the CID
async fn post_data(client: &Client, base_url: &str, data: &[u8]) -> String {
    let response = client
        .post(format!("{}/v1/data", base_url))
        .header("Content-Type", "application/octet-stream")
        .body(data.to_vec())
        .send()
        .await
        .expect("Failed to send POST request");

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        panic!("POST failed: {} - {}", status, body);
    }

    let create_response: serde_json::Value = response.json().await.expect("Failed to parse response");
    create_response["cid"].as_str().expect("No CID in response").to_string()
}

/// Helper to get data by CID
async fn get_data(client: &Client, base_url: &str, cid: &str) -> Vec<u8> {
    let response = client
        .get(format!("{}/v1/data/{}", base_url, cid))
        .send()
        .await
        .expect("Failed to send GET request");

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        panic!("GET /v1/data/{} failed: {} - {}", cid, status, body);
    }

    response.bytes().await.expect("Failed to read response body").to_vec()
}

#[tokio::test]
#[ignore = "Requires Azure credentials"]
async fn test_azure_put_and_get_data() {
    init_logging();

    let port = find_available_port().await;
    let _handle = start_test_server_azure(port).await;

    let client = Client::new();
    let base_url = format!("http://127.0.0.1:{}", port);

    // Test 1: Upload unique random data (guaranteed unique via random bytes)
    let random_bytes: [u8; 16] = rand::random();
    let unique_data = format!("unique test data {:?}", random_bytes);
    let unique_data = unique_data.as_bytes();

    let cid = post_data(&client, &base_url, unique_data).await;
    info!("Uploaded unique data with CID: {}", cid);

    let fetched = get_data(&client, &base_url, &cid).await;
    assert_eq!(fetched, unique_data, "Unique data mismatch");
    info!("Successfully round-tripped unique data ({} bytes)", unique_data.len());

    // Test 2: Upload the same data twice (should be idempotent)
    let duplicate_data = b"this is duplicate test data";

    let cid1 = post_data(&client, &base_url, duplicate_data).await;
    info!("First upload of duplicate data, CID: {}", cid1);

    let cid2 = post_data(&client, &base_url, duplicate_data).await;
    info!("Second upload of duplicate data, CID: {}", cid2);

    // Both uploads should produce the same CID (content-addressed)
    assert_eq!(cid1, cid2, "Same data should produce same CID");

    // Should still be able to fetch the data
    let fetched = get_data(&client, &base_url, &cid1).await;
    assert_eq!(fetched, duplicate_data, "Duplicate data mismatch");
    info!("Successfully handled duplicate upload");
}

#[tokio::test]
#[ignore = "Requires Azure credentials"]
async fn test_azure_list_routes() {
    init_logging();

    let port = find_available_port().await;
    let _handle = start_test_server_azure(port).await;

    let client = Client::new();
    let base_url = format!("http://127.0.0.1:{}", port);

    let response = client
        .get(format!("{}/v1/routes", base_url))
        .send()
        .await
        .expect("Failed to send GET request");

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        panic!("GET /v1/routes failed: {} - {}", status, body);
    }

    let routes_response: serde_json::Value = response.json().await.expect("Failed to parse response");
    info!("Routes response: {:?}", routes_response);
}
