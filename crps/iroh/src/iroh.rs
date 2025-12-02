use std::{pin::Pin, str::FromStr};

use anyhow::Result;
use async_trait::async_trait;
use bao_tree::io::BaoContentItem;
use bytes::Bytes;
use cid_router_core::{
    cid_filter::{CidFilter, CodeFilter},
    crp::{Crp, CrpCapabilities, ProviderType, RouteResolver},
    routes::Route,
    Context,
};
use futures::{Stream, StreamExt};
use iroh::{Endpoint, EndpointAddr, EndpointId};
use iroh_blobs::{get::request::GetBlobItem, ticket::BlobTicket, Hash};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug)]
pub struct IrohCrp {
    addr: EndpointAddr,
    endpoint: Endpoint,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IrohCrpConfig {
    pub node_addr_ref: IrohNodeAddrRef,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IrohNodeAddrRef {
    EndpointId(String),
    EndpointTicket(String),
    Ticket(String),
}

impl IrohCrp {
    pub async fn new_from_config(config: Value) -> Result<Self> {
        let IrohCrpConfig { node_addr_ref } = serde_json::from_value(config)?;

        let endpoint_addr = match node_addr_ref {
            IrohNodeAddrRef::EndpointId(node_id) => {
                let endpoint_id = EndpointId::from_str(&node_id)?;
                EndpointAddr::from(endpoint_id)
            }
            IrohNodeAddrRef::EndpointTicket(ticket) => {
                let ticket = iroh_tickets::endpoint::EndpointTicket::from_str(&ticket)?;
                ticket.endpoint_addr().to_owned()
            }
            IrohNodeAddrRef::Ticket(ticket) => {
                let ticket = BlobTicket::from_str(&ticket)?;
                ticket.addr().clone()
            }
        };

        let endpoint = Endpoint::bind().await?;

        Ok(Self {
            addr: endpoint_addr,
            endpoint,
        })
    }
}

#[async_trait]
impl Crp for IrohCrp {
    fn provider_id(&self) -> String {
        "iroh".to_string()
    }

    fn provider_type(&self) -> ProviderType {
        ProviderType::Iroh
    }

    async fn reindex(&self, _cx: &Context) -> anyhow::Result<()> {
        // TODO: Implement reindexing logic
        todo!();
    }

    fn capabilities<'a>(&'a self) -> CrpCapabilities<'a> {
        CrpCapabilities {
            route_resolver: Some(self),
            size_resolver: None, // TODO
        }
    }

    fn cid_filter(&self) -> CidFilter {
        CidFilter::MultihashCodeFilter(CodeFilter::Eq(0x1e)) // blake3
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
        let Self { addr, .. } = self;
        let cid = route.cid;

        let hash = cid.hash().digest();
        let hash: [u8; 32] = hash.try_into()?;
        let hash = Hash::from_bytes(hash);

        let conn = self
            .endpoint
            .connect(addr.clone(), iroh_blobs::ALPN)
            .await?;

        println!("get {:?} from {}", hash, addr.id.fmt_short());

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
