use std::num::NonZeroU32;
use std::pin::Pin;

use anyhow::{Result, anyhow};
use async_trait::async_trait;
use azure_storage::prelude::*;
use azure_storage_blobs::{blob::Blob, prelude::*};
use cid::Cid;
use futures::{Stream, StreamExt};
use iroh_blobs::BlobFormat;

use cid_router_core::{
    Context,
    cid::{Codec, blake3_hash_to_cid},
    cid_filter::CidFilter,
    crp::{BytesResolver, Crp, CrpCapabilities, ProviderType},
    db::{Direction, OrderBy},
    routes::{Route, RouteStub},
};

use crate::config::ContainerConfig;

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
            bytes_resolver: Some(self),
            size_resolver: None, // TODO
        }
    }

    fn cid_filter(&self) -> cid_router_core::cid_filter::CidFilter {
        CidFilter::None
    }
}

#[async_trait]
impl BytesResolver for Container {
    async fn get_bytes(
        &self,
        route: &Route,
        _auth: Vec<u8>, // TODO - support user-provided authentication
    ) -> Result<
        Pin<
            Box<
                dyn Stream<Item = Result<bytes::Bytes, Box<dyn std::error::Error + Send + Sync>>>
                    + Send,
            >,
        >,
        Box<dyn std::error::Error + Send + Sync>,
    > {
        let blob_name = Self::route_url_to_name(&route.route)?;
        let _client = self.client.blob_client(blob_name);
        todo!();
    }
}

impl Container {
    pub fn new(cfg: ContainerConfig) -> Self {
        let ContainerConfig {
            account, container, ..
        } = cfg.clone();
        // TODO: support credentials for private blob storage
        let credentials = StorageCredentials::anonymous();
        let client = BlobServiceClient::new(account, credentials);
        let client = client.container_client(container);

        Self { cfg, client }
    }

    // pub async fn update_blob_index(
    //     &self,
    //     cx: &Context,
    //     blob_storage_config: &BlobStorageConfig,
    // ) -> Result<()> {
    //     log::debug!("Updating blob index...");

    //     for container_cfg in &blob_storage_config.containers {
    //         self.add_stubs_for_missing_blobs(cx, container_cfg.clone())
    //             .await?;

    //         // self.prune_index_entries_for_deleted_or_filtered_blobs(account, container, filter)
    //         //     .await?;
    //     }

    //     log::debug!("Finished updating blob index.");

    //     Ok(())
    // }

    async fn add_stubs_for_missing_blobs(&self, cx: &Context) -> Result<()> {
        let response = self
            .client
            .list_blobs()
            .max_results(NonZeroU32::new(10 * 1000).unwrap())
            .into_stream()
            .next()
            .await
            .expect("stream failed")?;

        for blob in response.blobs.blobs() {
            if !self
                .cfg
                .filter
                .blob_is_match(&blob.name, blob.properties.content_length)
            {
                continue;
            }

            // let name = blob.name.clone();
            // let timestamp = blob.properties.last_modified.unix_timestamp();
            // let size = blob.properties.content_length;
            let url = self.blob_to_route_url(blob);

            if cx.db().routes_for_url(&url).await?.is_empty() {
                let stub = Route::builder(self)
                    .size(blob.properties.content_length)
                    .route(url)
                    .format(BlobFormat::Raw)
                    .build_stub()?;

                cx.db().insert_stub(&stub).await?;
            }
        }

        Ok(())
    }

    fn blob_to_route_url(&self, blob: &Blob) -> String {
        format!(
            "https://{}.blob.core.windows.net/{}/{}",
            self.cfg.account, self.cfg.container, blob.name
        )
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
        log::debug!("Updating blob index hashes...");

        let stubs = cx
            .db()
            .list_provider_stubs(&self.provider_id(), OrderBy::Size(Direction::Asc), 0, -1)
            .await?;

        for stub in stubs {
            let builder = stub.builder();
            let RouteStub { route, size, .. } = stub;
            let name = Self::route_url_to_name(&route)?;

            log::trace!(
                "Streaming blob to compute hash: container={} name={name}",
                &self.cfg.container
            );

            let hash = {
                let mut hasher = blake3::Hasher::new();

                if let Some(size) = size
                    && size == 0
                {
                    hasher.update(&[]);
                } else {
                    let blob_client = self.client.blob_client(&route);
                    let mut blob_stream = blob_client.get().into_stream();

                    while let Some(chunk_response) = blob_stream.next().await {
                        let chunk_response = chunk_response?;
                        let chunk = chunk_response.data.collect().await?;

                        hasher.update(&chunk);
                    }
                }
                hasher.finalize()
            };

            log::trace!("Computed hash={hash} for blob: name={name}",);

            let cid = blake3_hash_to_cid(hash.into(), Codec::Raw);

            let completed_route = builder.cid(cid).build(cx)?;

            cx.db().complete_stub(&completed_route).await?;
        }

        log::debug!("Finished updating blob index hashes.");

        Ok(())
    }

    // pub fn update_iroh_collections_index(
    //     &self,
    //     cx: &Context,
    //     blob_storage_config: &BlobStorageConfig,
    // ) -> Result<()> {
    //     log::debug!("Updating iroh collections index...");

    //     // get all routes in thie container
    //     let blobs = cx.db().list_provider_routes(
    //         &self.provider_id(),
    //         OrderBy::Size(Direction::Asc),
    //         0,
    //         -1,
    //     )?;

    //     // group blobs into collections they are a part of, blobs belong to multiple collections
    //     // if they have multiple parent directories (multiple slashes in their name)
    //     let collections_map = {
    //         let mut cs = MultiMap::new();

    //         for (name, blob_info) in &blobs {
    //             let mut parts = name.as_str().split('/').collect::<Vec<_>>();
    //             parts.pop(); // remove the filename

    //             let mut path = String::new();

    //             for part in parts {
    //                 path.push_str(part);

    //                 cs.insert(path.clone(), (name.as_str(), blob_info));

    //                 path.push('/');
    //             }
    //         }

    //         cs
    //     };

    //     // filter out collections containing blobs that aren't hashed yet
    //     let collections_map = collections_map
    //         .into_iter()
    //         .filter_map(|(path, blobs)| {
    //             let mut bs = vec![];
    //             for (name, blob_info) in blobs {
    //                 let hash = blob_info.hash;
    //                 let hash = hash?;
    //                 bs.push((name, (hash, blob_info)));
    //             }
    //             Some((path, bs))
    //         })
    //         .collect::<MultiMap<_, _>>();

    //     // compute iroh collection blobs
    //     let collections_blobs = collections_map
    //             .iter_all()
    //             .map(|(path, blobs)| {
    //                 let mut blobs = blobs
    //                     .iter()
    //                     .map(|(name, (hash, blob_info))| {
    //                         let name = name.strip_prefix(path).expect("failed to strip path prefix in a way that indicates collections indexer logic has a bug").to_owned();
    //                         let hash = Hash::from_bytes(*hash);
    //                         (name, hash, blob_info)
    //                     })
    //                     .collect::<Vec<_>>();

    //                 // alphabetical order of path names for collection sequence
    //                 blobs.sort_by(|(a, ..), (b, ..)| a.cmp(b));

    //                 let collection = Collection::from_iter(blobs.clone().into_iter().map(|(name, hash, ..)| (name, hash)));

    //                 let collection_blob = match collection.to_blobs().collect::<Vec<_>>().as_slice() {
    //                     [_meta_blob, collection_blob] => collection_blob.clone(),
    //                     bs => panic!("expected two blobs, found {}.", bs.len()),
    //                 };

    //                 let collection_hash: [u8; 32] = blake3::hash(&collection_blob).into();

    //                 let timestamp = blobs.iter().map(|(_, _, blob_info)| blob_info.timestamp).max().expect("expected at least one blob in a collection");
    //                 let size = blobs.iter().map(|(_, _, blob_info)| blob_info.size).sum::<u64>();

    //                 (path.to_owned(), collection_hash, (timestamp, size))
    //             })
    //             .collect::<Vec<_>>();

    //     // update iroh collection index
    //     // let wtx = self.db.begin_write()?;
    //     // {
    //     //     let mut collection_index_table = wtx.open_table(COLLECTION_INDEX_TABLE)?;
    //     //     let mut collection_hash_table = wtx.open_multimap_table(COLLECTION_HASH_INDEX_TABLE)?;

    //     for (path, collection_hash, (timestamp, size)) in &collections_blobs {
    //         let account = account.clone();
    //         let container = container.clone();

    //         let blob_id = BlobIdTuple::from(BlobId {
    //             account,
    //             container,
    //             name: path.clone(),
    //         });

    //         let existing_entry = {
    //             let rtx = self.db.begin_read()?;
    //             let table = rtx.open_table(COLLECTION_INDEX_TABLE)?;

    //             table.get(&blob_id)?
    //         };

    //         let now = chrono::Utc::now().timestamp();

    //         let blob_info = BlobInfoTuple::from(BlobInfo {
    //             timestamp: *timestamp,
    //             size: *size,
    //             hash: Some(*collection_hash),
    //             time_first_indexed: existing_entry
    //                 .map(|v| v.value())
    //                 .map(BlobInfo::from)
    //                 .map(|info| info.time_first_indexed)
    //                 .unwrap_or(now),
    //             time_last_checked: now,
    //         });

    //         collection_index_table.insert(&blob_id, blob_info)?;
    //         collection_hash_table.insert(collection_hash, blob_id)?;
    //     }
    //     // }
    //     // wtx.commit()?;

    //     // // prune any iroh collection paths no longer present in this container
    //     // let current_collection_paths = collections_blobs
    //     //     .iter()
    //     //     .map(|(path, ..)| path.clone())
    //     //     .collect::<Vec<_>>();

    //     // let rtx = self.db.begin_read()?;
    //     // let table_collection_paths = rtx
    //     //     .open_table(COLLECTION_INDEX_TABLE)?
    //     //     .iter()?
    //     //     .filter_map(|entry| {
    //     //         let (key, value) = entry.unwrap();
    //     //         let (blob_id, blob_info) =
    //     //             (BlobId::from(key.value()), BlobInfo::from(value.value()));

    //     //         if blob_id.account == *account && blob_id.container == *container {
    //     //             Some((blob_id, blob_info))
    //     //         } else {
    //     //             None
    //     //         }
    //     //     })
    //     //     .collect::<Vec<_>>();

    //     // for (blob_id, blob_info) in table_collection_paths {
    //     //     if !current_collection_paths.contains(&blob_id.name) {
    //     //         let blob_id = BlobIdTuple::from(blob_id);

    //     //         let wtx = self.db.begin_write()?;
    //     //         {
    //     //             let mut collection_index_table = wtx.open_table(COLLECTION_INDEX_TABLE)?;
    //     //             let mut collection_hash_table =
    //     //                 wtx.open_multimap_table(COLLECTION_HASH_INDEX_TABLE)?;

    //     //             collection_index_table.remove(&blob_id)?;
    //     //             if let Some(hash) = blob_info.hash {
    //     //                 collection_hash_table.remove(hash, blob_id)?;
    //     //             }
    //     //         }
    //     //     }
    //     // }

    //     log::debug!("Finished updating iroh collections index.");

    //     Ok(())
    // }

    async fn calculate_blob_cid(&self, stub: &RouteStub) -> Result<Cid> {
        let name = stub.route.clone();

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

    // async fn prune_index_entries_for_deleted_or_filtered_blobs(
    //     &self,
    //     account: impl Into<String>,
    //     container: impl Into<String>,
    //     filter: &ContainerBlobFilter,
    // ) -> Result<()> {
    //     let account = account.into();
    //     let container = container.into();

    //     // TODO: support credentials for private blob storage
    //     let storage_credentials = StorageCredentials::anonymous();

    //     let blob_service = BlobServiceClient::new(account.clone(), storage_credentials);
    //     let container_client = blob_service.container_client(container.clone());

    //     let response = container_client
    //         .list_blobs()
    //         .max_results(NonZeroU32::new(10 * 1000).unwrap())
    //         .into_stream()
    //         .next()
    //         .await
    //         .expect("stream failed")?;

    //     let blobs = response.blobs.blobs().collect::<Vec<_>>();

    //     let rtx = self.db.begin_read()?;
    //     let table = rtx.open_table(BLOB_INDEX_TABLE)?;

    //     for entry in table.iter()? {
    //         let (key, value) = entry?;
    //         let (blob_id, blob_info) = (BlobId::from(key.value()), BlobInfo::from(value.value()));

    //         // skip entries that don't belong to this account/container
    //         if blob_id.account != account || blob_id.container != container {
    //             continue;
    //         }

    //         // remove entry if it no longer is included by the filter
    //         if !filter.blob_is_match(&blob_id.name, blob_info.size) {
    //             self.delete_blob_index_entry(&blob_id)?;
    //         }

    //         // remove the entry if it no longer exists in the blob storage
    //         if !blobs.iter().any(|blob| blob.name != blob_id.name) {
    //             self.delete_blob_index_entry(&blob_id)?;
    //         }
    //     }

    //     Ok(())
    // }

    // fn delete_blob_index_entry(&self, blob_id: &BlobId) -> Result<()> {
    //     log::trace!(
    //         "Deleting blob entry: account={account} container={container} name={name}",
    //         account = blob_id.account,
    //         container = blob_id.container,
    //         name = blob_id.name,
    //     );

    //     let blob_id = BlobIdTuple::from(blob_id.clone());

    //     let wtx = self.db.begin_write()?;
    //     {
    //         let mut table = wtx.open_table(BLOB_INDEX_TABLE)?;
    //         let blob_info = table
    //             .get(blob_id.clone())?
    //             .map(|v| v.value())
    //             .map(BlobInfo::from)
    //             .expect("blob info not found");

    //         table.remove(blob_id.clone())?;

    //         if let BlobInfo {
    //             hash: Some(hash), ..
    //         } = blob_info
    //         {
    //             wtx.open_multimap_table(BLOB_HASH_INDEX_TABLE)?
    //                 .remove(hash, blob_id)?;
    //         }
    //     }
    //     wtx.commit()?;

    //     Ok(())
    // }
}
