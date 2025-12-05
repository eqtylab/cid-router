use std::{io, path::PathBuf, pin::Pin, str::FromStr};

use anyhow::Result;
use async_trait::async_trait;
use bao_tree::{
    io::{outboard::PreOrderMemOutboard, BaoContentItem},
    ChunkRanges,
};
use bytes::Bytes;
use cid::Cid;
use cid_router_core::{
    cid_filter::{CidFilter, CodeFilter},
    crp::{BlobWriter, Crp, CrpCapabilities, ProviderType, RouteResolver},
    routes::Route,
    Context,
};
use futures::{Stream, StreamExt};
use iroh::{endpoint::SendStream, Endpoint, NodeAddr, NodeId, SecretKey};
use iroh_blobs::{
    get::request::GetBlobItem,
    protocol::{ChunkRangesSeq, PushRequest, RequestType},
    store::IROH_BLOCK_SIZE,
    ticket::BlobTicket,
    Hash,
};
use irpc::util::WriteVarintExt;
use log::error;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug)]
pub struct IrohRemoteCrp {
    node_addr: NodeAddr,
    endpoint: Endpoint,
    allow_put: bool,
}

#[derive(Debug, Clone)]
pub struct IrohCrp {
    store: iroh_blobs::store::fs::FsStore,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IrohCrpConfig {
    /// Path to the directory where blobs are stored
    pub path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IrohRemoteCrpConfig {
    pub node_addr_ref: IrohNodeAddrRef,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IrohNodeAddrRef {
    NodeId(String),
    NodeTicket(String),
    Ticket(String),
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

impl IrohRemoteCrp {
    pub async fn new_from_config(config: Value, secret_key: SecretKey) -> Result<Self> {
        let IrohRemoteCrpConfig { node_addr_ref } = serde_json::from_value(config)?;

        let node_addr = match node_addr_ref {
            IrohNodeAddrRef::NodeId(node_id) => {
                let node_id = NodeId::from_str(&node_id)?;
                NodeAddr::from(node_id)
            }
            IrohNodeAddrRef::NodeTicket(ticket) => {
                let ticket = iroh_base::ticket::NodeTicket::from_str(&ticket)?;
                ticket.node_addr().to_owned()
            }
            IrohNodeAddrRef::Ticket(ticket) => {
                let ticket = BlobTicket::from_str(&ticket)?;
                ticket.node_addr().clone()
            }
        };

        let endpoint = Endpoint::builder()
            .discovery_n0()
            .secret_key(secret_key)
            .bind()
            .await?;

        Ok(Self {
            node_addr,
            endpoint,
            allow_put: true,
        })
    }
}

#[async_trait]
impl Crp for IrohRemoteCrp {
    fn provider_id(&self) -> String {
        "iroh".to_string()
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
            size_resolver: None, // TODO
            blob_writer: if self.allow_put { Some(self) } else { None },
        }
    }

    fn cid_filter(&self) -> CidFilter {
        CidFilter::MultihashCodeFilter(CodeFilter::Eq(0x1e)) // blake3
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
            size_resolver: None, // TODO
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

#[async_trait]
impl BlobWriter for IrohRemoteCrp {
    async fn put_blob(
        &self,
        _auth: Option<Bytes>,
        cid: &Cid,
        data: &[u8],
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        error!("Putting blob with cid: {}", cid);
        error!("I am: {:?}", self.endpoint.node_id());
        error!("Target: {:?}", self.node_addr.node_id);
        if !self.allow_put {
            return Err("Put operations are not allowed on this CRP".into());
        }
        if cid.hash().code() != 0x1e {
            return Err("Unsupported CID hash code; only blake3 is supported".into());
        }
        let hash: [u8; 32] = cid
            .hash()
            .digest()
            .try_into()
            .expect("blake3 hash must be 32 bytes");
        let hash = iroh_blobs::Hash::from_bytes(hash);
        error!("Connecting to node...");
        let conn = self
            .endpoint
            .connect(self.node_addr.clone(), iroh_blobs::ALPN)
            .await?;
        error!("Connected. Opening stream...");
        let (mut writer, mut reader) = conn.open_bi().await?;
        error!("Opened stream. Writing push request...");
        let request = PushRequest::new(hash, ChunkRangesSeq::root());
        let request = write_push_request(request, &mut writer).await?;
        let (hash, bao) = create_n0_bao(data, &ChunkRanges::all())?;
        if hash != request.hash {
            return Err("Computed hash does not match requested hash".into());
        }
        writer.write_all(&bao).await?;
        writer.finish()?;
        let res = reader.read_to_end(1024).await;
        if let Err(e) = res {
            error!("Error reading response: {}", e);
            return Err(Box::new(e));
        }
        error!("Blob put completed for cid: {}", cid);
        Ok(())
    }
}

/// TODO: make this available in iroh-blobs
async fn write_push_request(
    request: PushRequest,
    stream: &mut SendStream,
) -> anyhow::Result<PushRequest> {
    let mut request_bytes = Vec::new();
    request_bytes.push(RequestType::Push as u8);
    request_bytes.write_length_prefixed(&request).unwrap();
    stream.write_all(&request_bytes).await?;
    Ok(request)
}

/// TODO: move this to iroh-blobs
pub fn create_n0_bao(data: &[u8], ranges: &ChunkRanges) -> anyhow::Result<(Hash, Vec<u8>)> {
    let outboard = PreOrderMemOutboard::create(data, IROH_BLOCK_SIZE);
    let mut encoded = Vec::new();
    let size = data.len() as u64;
    encoded.extend_from_slice(&size.to_le_bytes());
    bao_tree::io::sync::encode_ranges_validated(data, &outboard, ranges, &mut encoded)?;
    Ok((outboard.root.into(), encoded))
}

#[async_trait]
impl RouteResolver for IrohRemoteCrp {
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
        let Self { node_addr, .. } = self;
        let cid = route.cid;

        let hash = cid.hash().digest();
        let hash: [u8; 32] = hash.try_into()?;
        let hash = Hash::from_bytes(hash);

        let conn = self
            .endpoint
            .connect(node_addr.clone(), iroh_blobs::ALPN)
            .await?;

        println!("get {:?} from {}", hash, node_addr.node_id.fmt_short());

        let res = iroh_blobs::get::request::get_blob(conn, hash);
        let res = res
            .take_while(|item| n0_future::future::ready(!matches!(item, GetBlobItem::Done(_))))
            .filter_map(|item| {
                n0_future::future::ready(match item {
                    GetBlobItem::Item(item) => match item {
                        BaoContentItem::Leaf(leaf) => Some(Ok(leaf.data)),
                        // TODO - I don't think this is right. returning None here
                        // will likely end the stream prematurely
                        BaoContentItem::Parent(_parent) => None,
                    },
                    // This is filtered out, only for compiler happiness
                    GetBlobItem::Done(_stats) => None,
                    GetBlobItem::Error(err) => Some(Err(
                        Box::new(err) as Box<dyn std::error::Error + Send + Sync>
                    )),
                })
            });

        Ok(Box::pin(res))
    }
}
