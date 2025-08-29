use std::str::FromStr;
use std::pin::Pin;
use bao_tree::io::BaoContentItem;

use futures::{Stream, StreamExt};
use anyhow::Result;
use async_trait::async_trait;
use cid::Cid;
use cid_filter::{CidFilter, CodeFilter};
use iroh::{Endpoint, NodeAddr, NodeId};
use iroh_blobs::{get::request::{get_verified_size, GetBlobItem}, ticket::BlobTicket, BlobFormat, BlobsProtocol, Hash};
use routes::{IntoRoute, IrohRouteMethod, Route};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{config::ProviderConfig, crp::{Crp, Resolver}};

#[derive(Debug)]
pub struct IrohCrp {
    node_addr: NodeAddr,
    config: ProviderConfig,
    endpoint: Endpoint,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IrohCrpConfig {
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
    pub async fn new_from_config(
        iroh_crp_config: IrohCrpConfig,
        config: ProviderConfig,
    ) -> Result<Self> {
        let IrohCrpConfig { node_addr_ref } = iroh_crp_config;

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

        let endpoint = Endpoint::builder().discovery_n0().bind().await?;

        Ok(Self {
            node_addr,
            config,
            endpoint,
        })
    }
}

#[async_trait]
impl Crp for IrohCrp {
    fn cid_filter(&self) -> CidFilter {
        CidFilter::MultihashCodeFilter(CodeFilter::Eq(0x1e)) // blake3
    }

    async fn get_routes_for_cid(&self, cid: &Cid) -> Result<Vec<Route>> {
        let Self { node_addr, .. } = self;

        let hash = cid.hash().digest();
        let hash: [u8; 32] = hash.try_into()?;
        let hash = Hash::from_bytes(hash);

        let connection = self
            .endpoint
            .connect(node_addr.clone(), iroh_blobs::protocol::ALPN)
            .await?;

        // TODO: this just checks the node has the last blake3 chunk of the blob,
        //       it's not guaranteed to have the full blob and/or any linked blobs
        let (size, _) = get_verified_size(&connection, &hash).await?;

        let metadata = None;

        let routes = if size > 0 {
            // TODO: how to determine blob format? for now just only supporting raw
            let blob_format = BlobFormat::Raw;

            let ticket = BlobTicket::new(node_addr.clone(), hash, blob_format).to_string();

            vec![IrohRouteMethod { ticket }.into_route(Some(self.provider_id()), metadata)?]
        } else {
            vec![]
        };

        Ok(routes)
    }

    fn provider_config(&self) -> Value {
        serde_json::to_value(&self.config).expect("unexpectedly failed to serialize a config type")
    }
}

#[async_trait]
impl Resolver for IrohCrp {
    async fn get(&self, cid: &Cid, _auth: Vec<u8>) -> Result<
        Pin<Box<dyn Stream<Item = Result<bytes::Bytes, Box<dyn std::error::Error + Send + Sync>>> + Send>>,
        Box<dyn std::error::Error + Send + Sync>
    > {
        let Self { node_addr, .. } = self;

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
            .take_while(|item| {
                n0_future::future::ready(!matches!(item, GetBlobItem::Done(_)))
            })
            .filter_map(|item| {
                n0_future::future::ready(match item {
                    GetBlobItem::Item(item) => match item {
                        BaoContentItem::Leaf(leaf) => {
                            Some(Ok(bytes::Bytes::from(leaf.data)))
                        }
                        // TODO - I don't think this is right. returning None here
                        // will likely end the stream prematurely
                        BaoContentItem::Parent(_parent) => {
                            None
                        }
                    },
                    // This is filtered out, only for compiler happiness
                    GetBlobItem::Done(_stats) => None,
                    GetBlobItem::Error(err) => Some(Err(Box::new(err) as Box<dyn std::error::Error + Send + Sync>)),
                })
            });

        Ok(Box::pin(res))
    }
}

// #[async_trait]
// impl SizeResolver for IrohCrp {
//     async fn get_size(&self, cid: &Cid) -> Result<u64> {
//         let Self { node_addr, .. } = self;

//         let hash = cid.hash().digest();
//         let hash: [u8; 32] = hash.try_into()?;
//         let hash = Hash::from_bytes(hash);

//         let connection = self
//             .endpoint
//             .connect(node_addr.clone(), iroh_blobs::ALPN)
//             .await?;

//         let (size, _) = get_verified_size(&connection, &hash).await?;

//         Ok(size)
//     }
// }
//

mod tests {
    use super::*;
    use cid::multihash::Multihash;
    use iroh_blobs::{store::mem::MemStore};
    use iroh::{protocol::Router, Watcher};


    struct Provider {
        blobs: BlobsProtocol,
        router: Router,
    }

    impl Provider {
        async fn new() -> Self {
            // make an iroh endpoint
            let endpoint = Endpoint::builder().discovery_n0().bind().await.unwrap();
            // initialize an in-memory backing store for iroh-blobs
            let store = MemStore::new();
            // initialize a struct that can accept blobs requests over iroh connections
            let blobs = BlobsProtocol::new(&store, endpoint.clone(), None);
            // For sending files we build a router that accepts blobs connections & routes them
            // to the blobs protocol.
            let router = Router::builder(endpoint)
                .accept(iroh_blobs::ALPN, blobs.clone())
                .spawn();

            // return both the router, and the blobs protocol
            Self { blobs, router }
        }
    }

    pub fn blake3_to_cid(hash: iroh_blobs::Hash) -> Cid {
        // construct a BLAKE3 multihash. This should never fail, because we
        // always use the bytes of a valid hash
        let mh =
            Multihash::wrap(0x1e, hash.as_bytes()).expect("invalid BLAKE3 hash");
        // encode multihash with the "raw" codec
        Cid::new_v1(0x55, mh)
    }

    #[tokio::test]
    async fn test_resolve() {
        // setup: create provider, add data, get hash & provider address
        let prov = Provider::new().await;
        let data = bytes::Bytes::from("oh hello there fren");
        let res = prov.blobs.add_bytes(data.clone()).await.unwrap();
        let cid = blake3_to_cid(res.hash);
        let prov_addr = prov.router.endpoint().node_addr().initialized().await;
        let ticket = iroh_base::ticket::NodeTicket::new(prov_addr.clone());
        println!("added {:?} to {}", &res.hash, ticket.node_addr().node_id.fmt_short());

        // create the CRP, point it at the provider
        let crp = IrohCrp::new_from_config(
            IrohCrpConfig { node_addr_ref: IrohNodeAddrRef::NodeTicket(ticket.to_string())},
            ProviderConfig::Iroh(IrohCrpConfig { node_addr_ref: IrohNodeAddrRef::NodeTicket(ticket.to_string()) })
        ).await.unwrap();

        // run a get
        let mut res = crp.get(&cid, vec![]).await.unwrap();

        // TODO: should be factored into a get_all wrapper func
        let mut buffer = bytes::BytesMut::new();
        while let Some(chunk) = res.next().await {
            let chunk = chunk.unwrap();
            buffer.extend_from_slice(&chunk);
        }
        let got = buffer.freeze();

        assert_eq!(got, data);
    }
}
