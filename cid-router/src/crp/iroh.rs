use std::str::FromStr;

use anyhow::Result;
use async_trait::async_trait;
use cid::Cid;
use cid_filter::{CidFilter, CodeFilter};
use iroh_base::{
    base32,
    hash::{BlobFormat, Hash},
    key::NodeId,
    node_addr::NodeAddr,
    ticket::BlobTicket,
};
use iroh_bytes::get::request::get_verified_size;
use iroh_net::{key::SecretKey, MagicEndpoint};
use routes::{IntoRoute, IrohRouteMethod, Route};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{config::ProviderConfig, crp::Crp};

#[derive(Debug)]
pub struct IrohCrp {
    node_addr: NodeAddr,
    config: ProviderConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IrohCrpConfig {
    pub node_addr_ref: IrohNodeAddrRef,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IrohNodeAddrRef {
    NodeId(String),
    Ticket(String),
}

impl IrohCrp {
    pub fn new_from_config(iroh_crp_config: IrohCrpConfig, config: ProviderConfig) -> Result<Self> {
        let IrohCrpConfig { node_addr_ref } = iroh_crp_config;

        let node_addr = match node_addr_ref {
            IrohNodeAddrRef::NodeId(node_id) => {
                let node_id_bytes = base32::parse_array(&node_id)?;
                let node_id = NodeId::from_bytes(&node_id_bytes)?;
                NodeAddr::from(node_id)
            }
            IrohNodeAddrRef::Ticket(ticket) => {
                let ticket = BlobTicket::from_str(&ticket)?;
                ticket.node_addr().clone()
            }
        };

        Ok(Self { node_addr, config })
    }
}

#[async_trait]
impl Crp for IrohCrp {
    async fn init(&mut self) -> Result<()> {
        Ok(())
    }

    fn cid_filter(&self) -> CidFilter {
        CidFilter::MultihashCodeFilter(CodeFilter::Eq(0x1e)) // blake3
    }

    async fn get_routes_for_cid(&self, cid: &Cid) -> Result<Vec<Route>> {
        let Self { node_addr, .. } = self;

        let secret_key = SecretKey::generate();

        let hash = cid.hash().digest();
        let hash: [u8; 32] = hash.try_into()?;
        let hash = Hash::from_bytes(hash);

        let endpoint = MagicEndpoint::builder()
            .alpns(vec![])
            .secret_key(secret_key)
            .bind(0)
            .await?;

        let connection = endpoint
            .connect(node_addr.clone(), iroh_bytes::protocol::ALPN)
            .await?;

        // TODO: this just checks the node has the last blake3 chunk of the blob,
        //       it's not guaranteed to have the full blob and/or any linked blobs
        let (size, _) = get_verified_size(&connection, &hash).await?;

        let metadata = None;

        let routes = if size > 0 {
            // TODO: how to determine blob format? for now just only supporting raw
            let blob_format = BlobFormat::Raw;

            let ticket = BlobTicket::new(node_addr.clone(), hash, blob_format)?.to_string();

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
