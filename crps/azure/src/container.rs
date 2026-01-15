use std::{num::NonZeroU32, pin::Pin, sync::Arc};

use anyhow::{Result, anyhow};
use async_trait::async_trait;
use azure_core::new_http_client;
use azure_identity::{ClientSecretCredential, TokenCredentialOptions};
use azure_storage::prelude::*;
use azure_storage_blobs::{blob::Blob, prelude::*};
use bytes::Bytes;
use cid::Cid;
use cid_router_core::{
    Context, Url,
    cid::{Codec, blake3_hash_to_cid},
    cid_filter::CidFilter,
    crp::{BlobWriter, Crp, CrpCapabilities, ProviderType, RouteResolver},
    db::{Direction, OrderBy},
    routes::{Route, RouteStub},
};
use futures::{Stream, StreamExt};
use log::info;

use crate::config::{ContainerConfig, Credentials};

/// An indexer can perform route indexing operations, scoped to a single azure
/// blob container.
#[derive(Debug, Clone)]
pub struct Container {
    cfg: ContainerConfig,
    client: ContainerClient,
}

#[async_trait]
impl Crp for Container {
    fn provider_id(&self) -> String {
        self.cfg.container.clone()
    }
    fn provider_type(&self) -> ProviderType {
        ProviderType::Azure
    }

    async fn reindex(&self, cx: &Context) -> anyhow::Result<()> {
        self.add_stubs_for_missing_blobs(cx).await?;
        self.update_blob_index_hashes(cx).await?;
        // TODO(b5): implement & call self.prune_entries here
        Ok(())
    }

    fn capabilities<'a>(&'a self) -> CrpCapabilities<'a> {
        CrpCapabilities {
            route_resolver: Some(self),
            blob_writer: if self.cfg.writeable { Some(self) } else { None },
        }
    }

    fn cid_filter(&self) -> cid_router_core::cid_filter::CidFilter {
        CidFilter::None
    }
}

#[async_trait]
impl BlobWriter for Container {
    async fn put_blob(
        &self,
        _auth: Option<bytes::Bytes>,
        cid: &Cid,
        data: &[u8],
    ) -> Result<Url, Box<dyn std::error::Error + Send + Sync>> {
        if !self.cfg.writeable {
            return Err("Container is not writeable".into());
        }
        info!("Uploading blob for cid {}...", cid);
        let name = cid.to_string();
        let blob_client = self.client.blob_client(&name);
        blob_client
            .put_block_blob(data.to_vec())
            .content_type("application/octet-stream")
            .await?;
        let url = self.name_to_route_url(&name);
        info!("Upload successful! Blob URL: {}", url);
        Ok(Url::parse(&url).unwrap())
    }
}

#[async_trait]
impl RouteResolver for Container {
    async fn get_bytes(
        &self,
        route: &Route,
        _auth: Option<Bytes>, // TODO - support user-provided authentication
    ) -> Result<
        Pin<
            Box<
                dyn Stream<Item = Result<bytes::Bytes, Box<dyn std::error::Error + Send + Sync>>>
                    + Send,
            >,
        >,
        Box<dyn std::error::Error + Send + Sync>,
    > {
        let name = Self::route_url_to_name(&route.url)?;
        let client = self.client.blob_client(&name);
        let stream = client.get().into_stream();

        // return a mapped stream that maps each chunk response to its data
        let mapped_stream = stream.then(|chunk_response| async move {
            match chunk_response {
                Ok(chunk) => chunk
                    .data
                    .collect()
                    .await
                    .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>),
                Err(e) => Err(Box::new(e) as Box<dyn std::error::Error + Send + Sync>),
            }
        });

        Ok(Box::pin(mapped_stream))
    }
}

impl Container {
    pub fn new(cfg: ContainerConfig) -> Self {
        let ContainerConfig {
            account,
            container,
            credentials,
            ..
        } = cfg.clone();
        info!(
            "Creating container client {account}:{container} with credentials: {}",
            credentials.is_some()
        );
        let credentials = match credentials {
            Some(c) => {
                let client = new_http_client();
                let Credentials {
                    tenant_id,
                    client_id,
                    client_secret,
                } = c;
                let credential = Arc::new(ClientSecretCredential::new(
                    client,
                    tenant_id,
                    client_id,
                    client_secret,
                    TokenCredentialOptions::default(),
                ));
                StorageCredentials::token_credential(credential)
            }
            None => {
                if let Ok(key) = std::env::var("AZURE_STORAGE_ACCESS_KEY") {
                    StorageCredentials::access_key(account.as_str(), key)
                } else {
                    StorageCredentials::anonymous()
                }
            }
        };
        let client = BlobServiceClient::new(account, credentials);
        let client = client.container_client(container);

        Self { cfg, client }
    }

    async fn add_stubs_for_missing_blobs(&self, cx: &Context) -> Result<()> {
        let response = self
            .client
            .list_blobs()
            .max_results(NonZeroU32::new(10 * 1000).unwrap())
            .into_stream()
            .next()
            .await
            .expect("stream failed")?;

        // TODO - check if results length is equal to max_results & paginate if so
        for blob in response.blobs.blobs() {
            if !self
                .cfg
                .filter
                .blob_is_match(&blob.name, blob.properties.content_length)
            {
                continue;
            }

            let url = self.blob_to_route_url(blob);

            if cx.db().routes_for_url(&url).await?.is_empty() {
                let stub = Route::builder(self)
                    .size(blob.properties.content_length)
                    .url(url)
                    .multicodec(Codec::Raw)
                    .build_stub()?;

                cx.db().insert_stub(&stub).await?;
            }
        }

        Ok(())
    }

    fn name_to_route_url(&self, name: &str) -> String {
        format!(
            "https://{}.blob.core.windows.net/{}/{}",
            self.cfg.account, self.cfg.container, name
        )
    }

    fn blob_to_route_url(&self, blob: &Blob) -> String {
        self.name_to_route_url(&blob.name)
    }

    fn route_url_to_name(url: &str) -> Result<String> {
        // Split by '/' and take everything after the container (4th segment onwards)
        let parts: Vec<&str> = url.split('/').collect();

        // URL format: https://{account}.blob.core.windows.net/{container}/{name}
        // parts[0] = "https:"
        // parts[1] = ""
        // parts[2] = "{account}.blob.core.windows.net"
        // parts[3] = "{container}"
        // parts[4..] = blob name parts

        if parts.len() >= 5 && parts[2].ends_with(".blob.core.windows.net") {
            Ok(parts[4..].join("/"))
        } else {
            Err(anyhow!("Invalid blob route URL"))
        }
    }

    pub async fn update_blob_index_hashes(&self, cx: &Context) -> Result<()> {
        info!("Updating blob index hashes...");

        let stubs = cx
            .db()
            .list_provider_stubs(&self.provider_id(), OrderBy::Size(Direction::Asc), 0, -1)
            .await?;

        for stub in stubs {
            let cid = self.calculate_blob_cid(&stub).await?;
            log::info!("Computed cid={cid} for blob: name={}", stub.url);
            let route = stub.builder().cid(cid).build(cx)?;
            cx.db().complete_stub(&route).await?;
        }

        log::debug!("Finished updating blob index hashes.");

        Ok(())
    }

    async fn calculate_blob_cid(&self, stub: &RouteStub) -> Result<Cid> {
        let name = Self::route_url_to_name(&stub.url)?;

        log::trace!("Streaming blob to compute hash: name={name}");

        let hash = {
            let mut hasher = blake3::Hasher::new();

            if let Some(size) = stub.size
                && size == 0
            {
                hasher.update(&[]);
            } else {
                let blob_client = self.client.blob_client(&name);
                let mut blob_stream = blob_client.get().into_stream();

                while let Some(chunk_response) = blob_stream.next().await {
                    let chunk_response = chunk_response?;
                    let chunk = chunk_response.data.collect().await?;

                    hasher.update(&chunk);
                }
            }

            hasher.finalize()
        };

        log::trace!("Computed hash={hash} for blob: name={name}");

        let cid = blake3_hash_to_cid(hash.into(), Codec::Raw);
        Ok(cid)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use azure_storage::StorageCredentials;
    use azure_storage_blobs::prelude::*;
    use cid::Cid;

    #[tokio::test]
    #[ignore]
    async fn create_data() -> anyhow::Result<()> {
        let account = std::env::var("AZURE_STORAGE_ACCOUNT")
            .expect("Set AZURE_STORAGE_ACCOUNT env var for this test");
        let access_key = std::env::var("AZURE_STORAGE_ACCESS_KEY")
            .expect("Set AZURE_STORAGE_ACCESS_KEY env var for this test");

        let credentials = StorageCredentials::access_key(&account, access_key);
        let service_client = BlobServiceClient::new(&account, credentials);
        let client = Arc::new(service_client);

        let container_name = "blobs";
        let container_client = client.container_client(container_name);

        // Create container (idempotent)
        let _ = container_client.create().await.or_else(|e| {
            match e.kind() {
                azure_core::error::ErrorKind::HttpResponse { status, .. } if *status == 409 => {
                    // Container already exists
                    Ok(())
                }
                _ => Err(e),
            }
        })?;

        // Some test data
        let data: &[u8] = b"Hello from a quick Rust test! This is blob content.";
        let cid_str = "bafkreigh2akiscaildcqabs2mfomphc4i7w3oecxi4n3go3ch3gaov2mqa"; // any valid CID
        let cid = Cid::try_from(cid_str).unwrap();

        let blob_name = cid.to_string();
        let blob_client = container_client.blob_client(&blob_name);

        println!("Uploading blob '{}' ({} bytes)...", blob_name, data.len());
        blob_client
            .put_block_blob(data)
            .content_type("text/plain; charset=utf-8")
            .await?;

        println!("Upload successful!");
        println!(
            "Blob URL: https://{}.blob.core.windows.net/{}/{}",
            account, container_name, blob_name
        );
        Ok(())
    }
}
