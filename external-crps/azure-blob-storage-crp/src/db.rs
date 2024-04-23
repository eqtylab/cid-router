use std::{cmp::Ordering, collections::HashMap, num::NonZeroU32, path::PathBuf};

use anyhow::Result;
use azure_storage::prelude::*;
use azure_storage_blobs::prelude::*;
use cid::{multihash::Multihash, Cid};
use futures::StreamExt;
use itertools::Itertools;
use redb::{MultimapTableDefinition, ReadableMultimapTable, ReadableTable, TableDefinition};
use tabled::{
    settings::{Alignment, Style},
    Table, Tabled,
};
use tokio::time::{sleep, Duration};

use crate::config::{BlobStorageConfig, ContainerBlobFilter, ContainerConfig};

type BlobIdTuple = (String, String, String); // (account, container, path)

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct BlobId {
    pub account: String,
    pub container: String,
    pub name: String,
}

impl From<BlobIdTuple> for BlobId {
    fn from(tuple: BlobIdTuple) -> Self {
        let (account, container, name) = tuple;
        Self {
            account,
            container,
            name,
        }
    }
}

impl From<BlobId> for BlobIdTuple {
    fn from(blob_id: BlobId) -> Self {
        let BlobId {
            account,
            container,
            name,
        } = blob_id;
        (account, container, name)
    }
}

type BlobInfoTuple = (i64, u64, Option<[u8; 32]>); // (timestamp, blob_size, hash)

#[derive(Debug, Clone)]
pub struct BlobInfo {
    pub timestamp: i64,
    pub size: u64,
    pub hash: Option<[u8; 32]>,
}

impl From<BlobInfoTuple> for BlobInfo {
    fn from(tuple: BlobInfoTuple) -> Self {
        let (timestamp, size, hash) = tuple;
        Self {
            timestamp,
            size,
            hash,
        }
    }
}

impl From<BlobInfo> for BlobInfoTuple {
    fn from(blob_info: BlobInfo) -> Self {
        let BlobInfo {
            timestamp,
            size,
            hash,
        } = blob_info;
        (timestamp, size, hash)
    }
}

type HashBytes = [u8; 32];

const BLOB_INDEX_TABLE: TableDefinition<BlobIdTuple, BlobInfoTuple> =
    TableDefinition::new("blob_index");

const HASH_INDEX_TABLE: MultimapTableDefinition<HashBytes, BlobIdTuple> =
    MultimapTableDefinition::new("hash_index");

pub struct Db {
    db: redb::Database,
}

impl Db {
    pub fn init(db_file: PathBuf) -> Result<Self> {
        let db = redb::Database::create(db_file)?;

        let tx = db.begin_write()?;
        {
            tx.open_table(BLOB_INDEX_TABLE)?;
            tx.open_multimap_table(HASH_INDEX_TABLE)?;
        }
        tx.commit()?;

        Ok(Self { db })
    }

    pub async fn update_blob_index(&self, blob_storage_config: &BlobStorageConfig) -> Result<()> {
        for ContainerConfig {
            account,
            container,
            filter,
        } in &blob_storage_config.containers
        {
            self.update_blob_index_for_container(account, container, filter)
                .await?;
        }

        Ok(())
    }

    pub async fn update_blob_index_for_container(
        &self,
        account: impl Into<String>,
        container: impl Into<String>,
        filter: &ContainerBlobFilter,
    ) -> Result<()> {
        let account = account.into();
        let container = container.into();

        // TODO: support credentials for private blob storage
        let storage_credentials = StorageCredentials::anonymous();

        let blob_service = BlobServiceClient::new(account.clone(), storage_credentials);
        let container_client = blob_service.container_client(container.clone());

        let response = container_client
            .list_blobs()
            .max_results(NonZeroU32::new(10 * 1000).unwrap())
            .into_stream()
            .next()
            .await
            .expect("stream failed")?;

        for blob in response.blobs.blobs() {
            let account = account.clone();
            let container = container.clone();
            let name = blob.name.clone();
            let timestamp = blob.properties.last_modified.unix_timestamp();
            let size = blob.properties.content_length;

            if !filter.blob_is_match(&name, size) {
                continue;
            }

            let blob_id = BlobId {
                account,
                container,
                name: name.clone(),
            };
            let blob_info = BlobInfo {
                timestamp,
                size,
                hash: None,
            };

            let current_blob_info = {
                let rtx = self.db.begin_read()?;
                let table = rtx.open_table(BLOB_INDEX_TABLE)?;

                table
                    .get(BlobIdTuple::from(blob_id.clone()))?
                    .map(|v| v.value())
                    .map(BlobInfo::from)
            };

            match current_blob_info {
                Some(BlobInfo {
                    timestamp: current_timestamp,
                    ..
                }) => {
                    match timestamp.cmp(&current_timestamp) {
                        Ordering::Equal => {
                            // indexed timestamp is up to date, do nothing
                        }
                        Ordering::Greater => {
                            log::trace!(
                                "updating blob entry: name={name} t={timestamp} size={size}"
                            );
                            self.update_blob_index_entry(blob_id, blob_info, current_blob_info)?;
                        }
                        Ordering::Less => {
                            log::error!("indexed timestamp is greater than current blob timestamp, for blob entry name={name}");
                            log::trace!(
                                "updating blob entry: name={name} t={timestamp} size={size}"
                            );
                            self.update_blob_index_entry(blob_id, blob_info, current_blob_info)?;
                        }
                    };
                }
                None => {
                    log::trace!("creating blob entry: name={name} t={timestamp} size={size}");
                    self.update_blob_index_entry(blob_id, blob_info, current_blob_info)?;
                }
            }
        }

        Ok(())
    }

    pub async fn update_blob_index_hashes(
        &self,
        blob_storage_config: &BlobStorageConfig,
    ) -> Result<()> {
        // TODO: will be needed for storage credentials
        let _ = blob_storage_config;

        // TODO: this isn't the best way to do things but for now is a nice way of leaving massive
        //       blobs until last
        for mb_size_cutoff in [
            // doubling sequence from 1MB to ~1TB
            1, 2, 4, 8, 16, 32, 64, 128, 256, 512, 1024, 2048, 4096, 8192, 16384, 32768, 65536,
            131072, 262144, 524288, 1048576,
        ] {
            log::trace!(
                "*** computing hashes for blobs <= {} MB ***",
                mb_size_cutoff
            );

            let rtx = self.db.begin_read()?;
            let table = rtx.open_table(BLOB_INDEX_TABLE)?;

            for entry in table.iter()? {
                let (key, value) = entry?;
                let (blob_id, blob_info) =
                    (BlobId::from(key.value()), BlobInfo::from(value.value()));

                let BlobId {
                    account,
                    container,
                    name,
                } = blob_id.clone();
                let BlobInfo { size, hash, .. } = blob_info;

                if size == 0 {
                    log::error!(
                        "blob size is 0, skipping: account={account} container={container} name={name}"
                    );
                    continue;
                }

                if size > mb_size_cutoff * 1024 * 1024 {
                    continue;
                }

                // skip if hash is already computed
                if hash.is_some() {
                    continue;
                }

                log::trace!(
                    "streaming blob to compute hash: size={size} account={account} container={container} name={name}"
                );

                let account = account.to_string();
                let container = container.to_string();
                let name = name.to_string();

                let storage_credentials = StorageCredentials::anonymous();
                let blob_service = BlobServiceClient::new(&account, storage_credentials);
                let container_client = blob_service.container_client(&container);
                let blob_client = container_client.blob_client(&name);
                let mut blob_stream = blob_client.get().into_stream();

                let hash = {
                    let mut hasher = blake3::Hasher::new();

                    while let Some(chunk_response) = blob_stream.next().await {
                        let chunk_response = chunk_response?;
                        let chunk = chunk_response.data.collect().await?;

                        hasher.update(&chunk);
                    }

                    hasher.finalize().as_bytes().to_owned()
                };

                log::trace!("computed hash={hash} for blob: account={account} container={container} name={name}", hash = hex::encode(hash));

                let new_blob_info = BlobInfo {
                    hash: Some(hash),
                    ..blob_info.clone()
                };

                self.update_blob_index_entry(blob_id, new_blob_info, Some(blob_info))?;
            }

            sleep(Duration::from_secs(1)).await;
        }

        Ok(())
    }

    fn update_blob_index_entry(
        &self,
        blob_id: BlobId,
        new_blob_info: BlobInfo,
        old_blob_info: Option<BlobInfo>,
    ) -> Result<()> {
        let BlobInfo { hash: new_hash, .. } = new_blob_info;

        let blob_id = BlobIdTuple::from(blob_id);
        let new_blob_info = BlobInfoTuple::from(new_blob_info);

        let wtx = self.db.begin_write()?;
        {
            let mut table = wtx.open_table(BLOB_INDEX_TABLE)?;
            table.insert(&blob_id, new_blob_info)?;

            // if present, remove the old hash from the hash index (for this blob id only)
            if let Some(BlobInfo {
                hash: Some(old_hash),
                ..
            }) = old_blob_info
            {
                wtx.open_multimap_table(HASH_INDEX_TABLE)?
                    .remove(old_hash, &blob_id)?;
            }

            // if present, insert the new hash into the hash index
            if let Some(new_hash) = new_hash {
                wtx.open_multimap_table(HASH_INDEX_TABLE)?
                    .insert(new_hash, blob_id)?;
            }
        }
        wtx.commit()?;

        Ok(())
    }
}

// TODO: re-org this a bit, split the view (hashes becoming cids for the table view) from the logic
//       probably have separate "db" entry type and "ascii table row" type
#[derive(Tabled)]
pub struct BlobEntryTableRow {
    pub size: u64,
    pub timestamp: i64,
    pub account: String,
    pub container: String,
    pub name: String,
    pub cid: String,
}

#[derive(Tabled)]
pub struct HashEntryTableRow {
    pub cid: String,
    pub account: String,
    pub container: String,
    pub name: String,
}

#[derive(Tabled)]
pub struct HashEntryWithBlobInfoTableRow {
    pub cid: String,
    pub size: u64,
    pub timestamp: i64,
    pub account: String,
    pub container: String,
    pub name: String,
}

impl Db {
    pub fn get_all_blob_entries(&self) -> Result<Vec<BlobEntryTableRow>> {
        let rtx = self.db.begin_read()?;
        let table = rtx.open_table(BLOB_INDEX_TABLE)?;

        let mut entries = Vec::new();

        for entry in table.iter()? {
            let (key, value) = entry?;
            let (key, value) = (key.value(), value.value());

            let (account, container, name) = key;
            let (timestamp, size, hash) = value;

            let cid = hash
                .map(|hash| {
                    let multihash = Multihash::wrap(0x1e, &hash)
                        .expect("unexpectedly failed to wrap a multihash");
                    Cid::new_v1(0x55, multihash).to_string()
                })
                .unwrap_or_default();

            let account = account.to_string();
            let container = container.to_string();
            let name = name.to_string();

            entries.push(BlobEntryTableRow {
                timestamp,
                size,
                account,
                container,
                name,
                cid,
            });
        }

        Ok(entries)
    }

    pub fn get_all_blob_entries_ascii_table(&self) -> Result<String> {
        let entries = self.get_all_blob_entries()?;

        let table = Table::new(entries)
            .with(Style::sharp())
            .with(Alignment::left())
            .to_string();

        Ok(table)
    }

    pub fn get_blob_ids_for_cid<T>(&self, cid: T) -> Result<Vec<BlobId>>
    where
        Cid: TryFrom<T, Error = cid::Error>,
    {
        let cid = Cid::try_from(cid)?;

        let hash: [u8; 32] = cid.hash().digest().try_into()?;

        let rtx = self.db.begin_read()?;
        let table = rtx.open_multimap_table(HASH_INDEX_TABLE)?;

        let mut entries = Vec::new();

        for blob_id in table.get(hash)? {
            let blob_id = blob_id?.value();

            entries.push(BlobId::from(blob_id));
        }

        Ok(entries)
    }

    fn get_all_hash_entry_groups(&self) -> Result<HashMap<HashBytes, Vec<BlobId>>> {
        let rtx = self.db.begin_read()?;
        let table = rtx.open_multimap_table(HASH_INDEX_TABLE)?;

        let mut groups = HashMap::new();

        for entry in table.iter()? {
            let (key, value) = entry?;

            let hash = key.value();

            let mut entries = Vec::new();

            for value in value {
                let value = value?;
                let blob_id = value.value().into();

                entries.push(blob_id);
            }

            groups.insert(hash, entries);
        }

        Ok(groups)
    }

    pub fn get_all_hash_entries(&self) -> Result<Vec<HashEntryTableRow>> {
        let mut entries = Vec::new();

        for (hash, blob_ids) in self.get_all_hash_entry_groups()?.into_iter().sorted() {
            let cid = {
                let multihash =
                    Multihash::wrap(0x1e, &hash).expect("unexpectedly failed to wrap a multihash");
                Cid::new_v1(0x55, multihash).to_string()
            };

            let n = blob_ids.len();
            for (i, blob_id) in blob_ids.into_iter().enumerate() {
                let cid = if i == 0 {
                    cid.clone()
                } else if i == (n - 1) {
                    "   └── DUPLICATE".to_owned()
                } else {
                    "   ├── DUPLICATE".to_owned()
                };

                let BlobId {
                    account,
                    container,
                    name,
                } = blob_id;

                entries.push(HashEntryTableRow {
                    cid,
                    account,
                    container,
                    name,
                });
            }
        }

        Ok(entries)
    }

    pub fn get_all_hash_entries_ascii_table(&self) -> Result<String> {
        let entries = self.get_all_hash_entries()?;

        let table = Table::new(entries)
            .with(Style::sharp())
            .with(Alignment::left())
            .to_string();

        Ok(table)
    }

    pub fn get_all_hash_entries_with_blob_info(
        &self,
    ) -> Result<Vec<HashEntryWithBlobInfoTableRow>> {
        let mut entries = Vec::new();

        for (hash, blob_ids) in self.get_all_hash_entry_groups()?.into_iter().sorted() {
            let cid = {
                let multihash =
                    Multihash::wrap(0x1e, &hash).expect("unexpectedly failed to wrap a multihash");
                Cid::new_v1(0x55, multihash).to_string()
            };

            let n = blob_ids.len();
            for (i, blob_id) in blob_ids.into_iter().enumerate() {
                let cid = if i == 0 {
                    cid.clone()
                } else if i == (n - 1) {
                    "   └── DUPLICATE".to_owned()
                } else {
                    "   ├── DUPLICATE".to_owned()
                };

                let rtx = self.db.begin_read()?;
                let table = rtx.open_table(BLOB_INDEX_TABLE)?;

                let blob_info = table
                    .get(BlobIdTuple::from(blob_id.clone()))?
                    .map(|v| v.value())
                    .map(BlobInfo::from)
                    .expect("blob info not found");

                let BlobId {
                    account,
                    container,
                    name,
                } = blob_id;

                let BlobInfo {
                    size, timestamp, ..
                } = blob_info;

                entries.push(HashEntryWithBlobInfoTableRow {
                    cid,
                    size,
                    timestamp,
                    account,
                    container,
                    name,
                });
            }
        }

        Ok(entries)
    }

    pub fn get_all_hash_entries_with_blob_info_ascii_table(&self) -> Result<String> {
        let entries = self.get_all_hash_entries_with_blob_info()?;

        let table = Table::new(entries)
            .with(Style::sharp())
            .with(Alignment::left())
            .to_string();

        Ok(table)
    }
}
