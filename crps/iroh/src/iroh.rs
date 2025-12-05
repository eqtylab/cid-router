use std::{io, path::PathBuf, pin::Pin};

use anyhow::Result;
use async_trait::async_trait;
use bytes::Bytes;
use cid::Cid;
use cid_router_core::{
    cid_filter::{CidFilter, CodeFilter},
    crp::{BlobWriter, Crp, CrpCapabilities, ProviderType, RouteResolver},
    routes::Route,
    Context,
};
use futures::Stream;
use iroh_blobs::Hash;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct IrohCrp {
    store: iroh_blobs::store::fs::FsStore,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IrohCrpConfig {
    /// Path to the directory where blobs are stored
    pub path: PathBuf,
}

impl IrohCrp {
    pub async fn new_from_config(config: IrohCrpConfig) -> io::Result<Self> {
        let path = if config.path.is_absolute() {
            config.path
        } else {
            std::env::current_dir()?.join(config.path)
        };
        let store = iroh_blobs::store::fs::FsStore::load(path)
            .await
            .map_err(|e| io::Error::other(e))?;
        Ok(Self { store })
    }
}

#[async_trait]
impl Crp for IrohCrp {
    fn provider_id(&self) -> String {
        "iroh_inline".to_string()
    }

    fn provider_type(&self) -> ProviderType {
        ProviderType::Iroh
    }

    async fn reindex(&self, _cx: &Context) -> anyhow::Result<()> {
        // TODO: Implement reindexing logic
        Ok(())
    }

    fn capabilities<'a>(&'a self) -> CrpCapabilities<'a> {
        CrpCapabilities {
            route_resolver: Some(self),
            blob_writer: Some(self),
        }
    }

    fn cid_filter(&self) -> CidFilter {
        CidFilter::MultihashCodeFilter(CodeFilter::Eq(0x1e)) // blake3
    }
}

#[async_trait]
impl BlobWriter for IrohCrp {
    async fn put_blob(
        &self,
        _auth: Option<Bytes>,
        cid: &Cid,
        data: &[u8],
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let blobs = self.store.blobs().clone();
        let data = Bytes::copy_from_slice(data);
        if cid.hash().code() != 0x1e {
            return Err("Unsupported CID hash code; only blake3 is supported".into());
        }
        blobs.add_bytes(data).with_tag().await.map_err(Box::new)?;
        Ok(())
    }
}

#[async_trait]
impl RouteResolver for IrohCrp {
    async fn get_bytes(
        &self,
        route: &Route,
        _auth: Option<Bytes>,
    ) -> Result<
        Pin<
            Box<
                dyn Stream<Item = Result<bytes::Bytes, Box<dyn std::error::Error + Send + Sync>>>
                    + Send,
            >,
        >,
        Box<dyn std::error::Error + Send + Sync>,
    > {
        let cid = route.cid;
        let hash = cid.hash().digest();
        let hash: [u8; 32] = hash.try_into()?;
        let hash = Hash::from_bytes(hash);
        let data = self.store.blobs().get_bytes(hash).await.map_err(Box::new)?;
        let stream = futures::stream::once(async move { Ok(data) });
        Ok(Box::pin(stream))
    }
}
