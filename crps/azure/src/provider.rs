use std::{pin::Pin, str::FromStr};

use anyhow::Result;
use async_trait::async_trait;
use bao_tree::io::BaoContentItem;
use cid::Cid;
use cid_router_core::{
    cid_filter::{CidFilter, CodeFilter},
    crp::{BytesResolver, Crp, CrpCapabilities, RoutesResolver},
    routes::{IntoRoute, IrohRouteMethod, Route},
};
use crp_iroh::IrohNodeAddrRef;
use futures::{Stream, StreamExt};
use iroh::{Endpoint, NodeAddr, NodeId};
use iroh_blobs::{
    BlobFormat, Hash,
    get::request::{GetBlobItem, get_verified_size},
    ticket::BlobTicket,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug)]
pub struct IrohCrp {
    node_addr: NodeAddr,
    endpoint: Endpoint,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AzureCrpConfig {
    pub node_addr_ref: IrohNodeAddrRef,
}

impl IrohCrp {
    pub async fn new_from_config(config: Value) -> Result<Self> {
        let AzureCrpConfig { node_addr_ref } = serde_json::from_value(config)?;

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
            endpoint,
        })
    }
}

#[async_trait]
impl Crp for IrohCrp {
    fn capabilities<'a>(&'a self) -> CrpCapabilities<'a> {
        CrpCapabilities {
            routes_resolver: Some(self),
            bytes_resolver: Some(self),
            size_resolver: None, // TODO
        }
    }

    fn cid_filter(&self) -> CidFilter {
        CidFilter::MultihashCodeFilter(CodeFilter::Eq(0x1e)) // blake3
    }
}

#[async_trait]
impl RoutesResolver for IrohCrp {
    async fn get_routes(&self, cid: &Cid) -> Result<Vec<Route>> {
        let Self { node_addr, .. } = &self;

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

            vec![IrohRouteMethod { ticket }.into_route(None, metadata)?]
        } else {
            vec![]
        };

        Ok(routes)
    }
}
