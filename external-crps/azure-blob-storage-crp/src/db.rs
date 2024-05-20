use std::{collections::HashMap, num::NonZeroU32, path::PathBuf};

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

type BlobInfoTuple = (i64, u64, Option<[u8; 32]>, i64, i64); // (timestamp, blob_size, hash, time_first_indexed, time_last_checked)

#[derive(Debug, Clone)]
pub struct BlobInfo {
    pub timestamp: i64,
    pub size: u64,
    pub hash: Option<[u8; 32]>,
    pub time_first_indexed: i64,
    pub time_last_checked: i64,
}

impl From<BlobInfoTuple> for BlobInfo {
    fn from(tuple: BlobInfoTuple) -> Self {
        let (timestamp, size, hash, time_first_indexed, time_last_checked) = tuple;
        Self {
            timestamp,
            size,
            hash,
            time_first_indexed,
            time_last_checked,
        }
    }
}

impl From<BlobInfo> for BlobInfoTuple {
    fn from(blob_info: BlobInfo) -> Self {
        let BlobInfo {
            timestamp,
            size,
            hash,
            time_first_indexed,
            time_last_checked,
        } = blob_info;
        (timestamp, size, hash, time_first_indexed, time_last_checked)
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
            self.add_index_entries_for_missing_blobs(account, container, filter)
                .await?;

            self.prune_index_entries_for_deleted_or_filtered_blobs(account, container, filter)
                .await?;
        }

        Ok(())
    }

    pub async fn add_index_entries_for_missing_blobs(
        &self,
        account: impl Into<String>,
        container: impl Into<String>,
        filter: &ContainerBlobFilter,
    ) -> Result<()> {
        let account = account.into();
        let container = container.into();

        log::debug!(
            "adding index entries for missing blobs, account={account}, container={container}"
        );

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

            let current_blob_info = {
                let rtx = self.db.begin_read()?;
                let table = rtx.open_table(BLOB_INDEX_TABLE)?;

                table
                    .get(BlobIdTuple::from(blob_id.clone()))?
                    .map(|v| v.value())
                    .map(BlobInfo::from)
            };

            if current_blob_info.is_none() {
                let now = chrono::Utc::now().timestamp();

                let new_blob_info = BlobInfo {
                    timestamp,
                    size,
                    hash: None,
                    time_first_indexed: now,
                    time_last_checked: now,
                };

                self.update_blob_index_entry(blob_id, new_blob_info, None)?;
            }
        }

        Ok(())
    }

    pub async fn prune_index_entries_for_deleted_or_filtered_blobs(
        &self,
        account: impl Into<String>,
        container: impl Into<String>,
        filter: &ContainerBlobFilter,
    ) -> Result<()> {
        let account = account.into();
        let container = container.into();

        log::debug!("pruning index entries for deleted or filtered blobs, account={account}, container={container}");

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

        let blobs = response.blobs.blobs().collect::<Vec<_>>();

        let rtx = self.db.begin_read()?;
        let table = rtx.open_table(BLOB_INDEX_TABLE)?;

        for entry in table.iter()? {
            let (key, value) = entry?;
            let (blob_id, blob_info) = (BlobId::from(key.value()), BlobInfo::from(value.value()));

            // skip entries that don't belong to this account/container
            if blob_id.account != account || blob_id.container != container {
                continue;
            }

            // remove entry if it no longer is included by the filter
            if !filter.blob_is_match(&blob_id.name, blob_info.size) {
                self.delete_blob_index_entry(&blob_id)?;
            }

            // remove the entry if it no longer exists in the blob storage
            if !blobs.iter().any(|blob| blob.name != blob_id.name) {
                self.delete_blob_index_entry(&blob_id)?;
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

                let hash = {
                    let mut hasher = blake3::Hasher::new();

                    if size == 0 {
                        hasher.update(&[]);
                    } else {
                        let storage_credentials = StorageCredentials::anonymous();
                        let blob_service = BlobServiceClient::new(&account, storage_credentials);
                        let container_client = blob_service.container_client(&container);
                        let blob_client = container_client.blob_client(&name);
                        let mut blob_stream = blob_client.get().into_stream();

                        while let Some(chunk_response) = blob_stream.next().await {
                            let chunk_response = chunk_response?;
                            let chunk = chunk_response.data.collect().await?;

                            hasher.update(&chunk);
                        }
                    }

                    hasher.finalize().as_bytes().to_owned()
                };

                log::trace!("computed hash={hash} for blob: account={account} container={container} name={name}", hash = hex::encode(hash));

                let now = chrono::Utc::now().timestamp();

                let new_blob_info = BlobInfo {
                    hash: Some(hash),
                    time_last_checked: now,
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
        current_blob_info: Option<BlobInfo>,
    ) -> Result<()> {
        log::trace!(
            "{action} blob entry: account={account} container={container} name={name} t={timestamp} size={size}",
            action = if current_blob_info.is_some() { "updating" } else { "creating" },
            account = blob_id.account,
            container = blob_id.container,
            name = blob_id.name,
            timestamp = new_blob_info.timestamp,
            size = new_blob_info.size,
        );

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
            }) = current_blob_info
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

    fn delete_blob_index_entry(&self, blob_id: &BlobId) -> Result<()> {
        log::trace!(
            "deleting blob entry: account={account} container={container} name={name}",
            account = blob_id.account,
            container = blob_id.container,
            name = blob_id.name,
        );

        let blob_id = BlobIdTuple::from(blob_id.clone());

        let wtx = self.db.begin_write()?;
        {
            let mut table = wtx.open_table(BLOB_INDEX_TABLE)?;
            let blob_info = table
                .get(blob_id.clone())?
                .map(|v| v.value())
                .map(BlobInfo::from)
                .expect("blob info not found");

            table.remove(blob_id.clone())?;

            if let BlobInfo {
                hash: Some(hash), ..
            } = blob_info
            {
                wtx.open_multimap_table(HASH_INDEX_TABLE)?
                    .remove(hash, blob_id)?;
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
    pub time_first_indexed: i64,
    pub time_last_checked: i64,
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
    pub time_first_indexed: i64,
    pub time_last_checked: i64,
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
            let (timestamp, size, hash, time_first_indexed, time_last_checked) = value;

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
                time_first_indexed,
                time_last_checked,
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

    pub fn get_blob_ids_and_infos_for_cid<T>(&self, cid: T) -> Result<Vec<(BlobId, BlobInfo)>>
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

            let rtx = self.db.begin_read()?;
            let table = rtx.open_table(BLOB_INDEX_TABLE)?;

            let blob_info = table
                .get(BlobIdTuple::from(blob_id.clone()))?
                .map(|v| v.value())
                .map(BlobInfo::from)
                .expect("blob info not found");

            entries.push((BlobId::from(blob_id), blob_info));
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
                    size,
                    timestamp,
                    time_first_indexed,
                    time_last_checked,
                    ..
                } = blob_info;

                entries.push(HashEntryWithBlobInfoTableRow {
                    cid,
                    size,
                    timestamp,
                    account,
                    container,
                    name,
                    time_first_indexed,
                    time_last_checked,
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
