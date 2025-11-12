use std::{fmt::Debug, pin::Pin, sync::Arc};

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

impl std::str::FromStr for ProviderType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "iroh" => Ok(ProviderType::Iroh),
            "azure" => Ok(ProviderType::Azure),
            _ => Err(format!("Unknown provider: {}", s)),
        }
    }
}

impl std::fmt::Display for ProviderType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let str = match self {
            ProviderType::Iroh => "iroh",
            ProviderType::Azure => "azure",
        };
        write!(f, "{}", str)
    }
}

/// CID Route Provider (CRP) Trait
#[async_trait]
pub trait Crp: Send + Sync + Debug {
    fn provider_id(&self) -> String;
    fn provider_type(&self) -> ProviderType;
    async fn reindex(&self, cx: &Context) -> Result<()>;

    fn capabilities<'a>(&'a self) -> CrpCapabilities<'a>;

    fn cid_filter(&self) -> CidFilter;

    fn provider_is_eligible_for_cid(&self, cid: &Cid) -> bool {
        self.cid_filter().is_match(cid)
    }
}

#[async_trait]
impl Crp for Arc<dyn Crp> {
    fn provider_id(&self) -> String {
        self.as_ref().provider_id()
    }
    fn provider_type(&self) -> ProviderType {
        self.as_ref().provider_type()
    }
    async fn reindex(&self, cx: &Context) -> Result<()> {
        self.as_ref().reindex(cx).await
    }

    fn capabilities<'a>(&'a self) -> CrpCapabilities<'a> {
        self.as_ref().capabilities()
    }

    fn cid_filter(&self) -> CidFilter {
        self.as_ref().cid_filter()
    }
}

/// All capabilities a CRP may have represented as self-referential trait objects.
pub struct CrpCapabilities<'a> {
    pub route_resolver: Option<&'a dyn RouteResolver>,
    pub size_resolver: Option<&'a dyn SizeResolver>,
    pub blob_writer: Option<&'a dyn BlobWriter>,
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

/// A RouteResolver can dereference a route, turning it into a stream of bytes, accepting
/// authentication data.
#[async_trait]
pub trait BlobWriter: Send + Sync {

    /// Puts a blob into the CRP, given optional authentication data, a CID, and the data bytes.
    /// 
    /// Note that this assumes that the data fits in memory, which is probably the case for most
    /// data that eqty wants to write. If this becomes a problem, we will add a second method that
    /// takes a stream of bytes instead.
    async fn put_blob(
        &self,
        auth: Option<bytes::Bytes>,
        cid: &Cid,
        data: &[u8],
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
}
