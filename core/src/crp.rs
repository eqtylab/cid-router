use std::pin::Pin;

use anyhow::Result;
use async_trait::async_trait;
use cid::Cid;
use futures::Stream;
use serde::{Deserialize, Serialize};

use crate::{cid_filter::CidFilter, routes::Route, Context};

/// Set of all supported CID Route Providers (CRPs) throughout the system
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Hash, Eq)]
pub enum ProviderType {
    Iroh,
    Azure,
}

impl ProviderType {
    pub fn to_string(&self) -> String {
        match self {
            ProviderType::Iroh => "iroh".to_string(),
            ProviderType::Azure => "azure".to_string(),
        }
    }

    pub fn from_str(s: &str) -> Result<Self, String> {
        match s {
            "iroh" => Ok(ProviderType::Iroh),
            "azure" => Ok(ProviderType::Azure),
            _ => Err(format!("Unknown provider: {}", s)),
        }
    }
}

/// CID Route Provider (CRP) Trait
#[async_trait]
pub trait Crp: Send + Sync {
    fn provider_id(&self) -> String;
    fn provider_type(&self) -> ProviderType;
    async fn reindex(&self, cx: &Context) -> Result<()>;

    fn capabilities<'a>(&'a self) -> CrpCapabilities<'a>;

    fn cid_filter(&self) -> CidFilter;

    fn provider_is_eligible_for_cid(&self, cid: &Cid) -> bool {
        self.cid_filter().is_match(cid)
    }
}

/// All capabilities a CRP may have represented as self-referential trait objects.
pub struct CrpCapabilities<'a> {
    pub route_resolver: Option<&'a dyn RouteResolver>,
    pub size_resolver: Option<&'a dyn SizeResolver>,
}

/// A RouteResolver can dereference a route, turning it into a stream of bytes, accepting
/// authentication data.
#[async_trait]
pub trait RouteResolver {
    async fn get_bytes(
        &self,
        route: &Route,
        auth: Option<bytes::Bytes>,
    ) -> Result<
        Pin<
            Box<
                dyn Stream<Item = Result<bytes::Bytes, Box<dyn std::error::Error + Send + Sync>>>
                    + Send,
            >,
        >,
        Box<dyn std::error::Error + Send + Sync>,
    >;
}

/// A SizeResolver can return the length in bytes of the blob a CID points at.
/// This is useful both as a preflight check before downloading a CID,
/// and as a fast means of checking if a CRP has the CID in the first place.
#[async_trait]
pub trait SizeResolver {
    async fn get_size(
        &self,
        cid: &Cid,
        auth: Vec<u8>,
    ) -> Result<u64, Box<dyn std::error::Error + Send + Sync>>;
}
