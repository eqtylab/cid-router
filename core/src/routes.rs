use anyhow::anyhow;
use cid::{Cid, CidGeneric};
use iroh::PublicKey;
use iroh_blobs::BlobFormat;
use serde::{Deserialize, Serialize};
use time::{format_description::well_known::Rfc3339, OffsetDateTime as DateTime};
use uuid::Uuid;

use crate::{
    context::Signer,
    crp::{Crp, ProviderType},
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Route {
    pub id: Uuid,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: DateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub verified_at: DateTime,
    pub provider_id: String,
    pub provider_type: ProviderType,
    pub url: String,
    pub cid: CidGeneric<64>,
    pub size: u64,
    pub blob_format: BlobFormat,
    pub creator: PublicKey, // PublicKey or DID
    pub signature: Vec<u8>,
}

impl Route {
    pub fn builder(provider: &impl Crp) -> RouteBuilder {
        RouteBuilder::new(provider)
    }

    pub(crate) fn from_sql_row(row: &rusqlite::Row<'_>) -> Result<Route, rusqlite::Error> {
        // TODO(b5) - remove unwraps!
        let id = row.get::<_, String>(0)?;
        let id = Uuid::parse_str(&id).unwrap();

        let data = row.get::<_, Vec<u8>>(6)?;
        let cid = Cid::try_from(data).unwrap();

        let blob_format_str: String = row.get(8)?;
        let blob_format = match blob_format_str.as_str() {
            "Raw" => BlobFormat::Raw,
            "HashSeq" => BlobFormat::HashSeq,
            _ => BlobFormat::Raw, // default fallback
        };

        let pub_key = row.get::<_, [u8; 32]>(9)?;
        // TODO(b5) - remove unwarp
        let creator = PublicKey::from_bytes(&pub_key).unwrap();

        Ok(Route {
            id,
            created_at: DateTime::parse(&row.get::<_, String>(1)?, &Rfc3339).unwrap(),
            verified_at: DateTime::parse(&row.get::<_, String>(2)?, &Rfc3339).unwrap(),
            provider_id: row.get::<_, String>(3)?,
            provider_type: ProviderType::from_str(&row.get::<_, String>(4)?).unwrap(),
            url: row.get(5)?,
            cid,
            size: row.get::<_, i64>(7)? as u64,
            blob_format,
            creator,
            signature: row.get(10)?,
        })
    }
}

/// state machine for building either a route or a stub
pub struct RouteBuilder {
    id: Uuid,
    provider_id: String,
    provider_type: ProviderType,
    cid: Option<Cid>,
    size: Option<u64>,
    url: Option<String>,
    blob_format: Option<BlobFormat>,
}

impl RouteBuilder {
    fn new(provider: &impl Crp) -> Self {
        Self {
            id: Uuid::new_v4(),
            provider_id: provider.provider_id(),
            provider_type: provider.provider_type(),
            cid: None,
            size: None,
            url: None,
            blob_format: None,
        }
    }

    pub fn cid(mut self, cid: Cid) -> Self {
        self.cid = Some(cid);
        self
    }

    pub fn size(mut self, size: u64) -> Self {
        self.size = Some(size);
        self
    }

    pub fn url(mut self, route: impl Into<String>) -> Self {
        self.url = Some(route.into());
        self
    }

    pub fn format(mut self, format: BlobFormat) -> Self {
        self.blob_format = Some(format);
        self
    }

    pub fn build_stub(self) -> anyhow::Result<RouteStub> {
        let route = self.url.ok_or_else(|| anyhow!("route is required"))?;
        let now = DateTime::now_utc();
        Ok(RouteStub {
            id: Uuid::new_v4(),
            created_at: now,
            verified_at: now,
            provider_id: self.provider_id,
            provider_type: self.provider_type,
            blob_format: self.blob_format,
            size: self.size,
            url: route,
        })
    }

    pub fn build(&self, signer: &impl Signer) -> anyhow::Result<Route> {
        let cid = self.cid.ok_or_else(|| anyhow!("cid is required"))?;
        let size = self.size.ok_or_else(|| anyhow!("size is required"))?;
        let route = self
            .url
            .clone()
            .ok_or_else(|| anyhow!("route is required"))?;
        let blob_format = self
            .blob_format
            .ok_or_else(|| anyhow!("format is required"))?;
        let signature = sign_route(signer, cid, size, &route, blob_format);

        let now = DateTime::now_utc();

        Ok(Route {
            id: self.id,
            created_at: now,
            verified_at: now,
            provider_id: self.provider_id.clone(),
            provider_type: self.provider_type.clone(),
            cid,
            size,
            url: route,
            blob_format,
            signature,
            creator: signer.public_key(),
        })
    }
}

fn sign_route(
    _signer: &impl Signer,
    _cid: Cid,
    _size: u64,
    _route: &str,
    _format: BlobFormat,
) -> Vec<u8> {
    // TODO - finish for real: serialize these values, hash them, and sign hash
    vec![]
}

/// A Route Stub is a partially-completed route. The core use case here is a
/// two-step indexing process, where a route is first created with a stub, and
/// then completed with a full route once the content CID can be calculated &
/// the route can be signed.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RouteStub {
    pub id: Uuid,
    pub provider_id: String,
    pub provider_type: ProviderType,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: DateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub verified_at: DateTime,
    pub blob_format: Option<BlobFormat>,
    pub size: Option<u64>,
    pub url: String,
}

impl RouteStub {
    // get a builder from a stub, so it can be completed
    pub fn builder(&self) -> RouteBuilder {
        RouteBuilder {
            id: self.id,
            provider_id: self.provider_id.clone(),
            provider_type: self.provider_type.clone(),
            cid: None,
            size: self.size,
            url: Some(self.url.clone()),
            blob_format: self.blob_format,
        }
    }

    pub(crate) fn from_sql_row(row: &rusqlite::Row<'_>) -> Result<RouteStub, rusqlite::Error> {
        // TODO(b5) - remove unwraps!
        let id = row.get::<_, String>(0)?;
        let id = Uuid::parse_str(&id).unwrap();
        let size = row.get::<_, Option<u64>>(7)?;
        let blob_format = row.get::<_, Option<String>>(8)?;
        let blob_format = blob_format_from_sql(blob_format);

        Ok(RouteStub {
            id,
            created_at: DateTime::parse(&row.get::<_, String>(1)?, &Rfc3339).unwrap(),
            verified_at: DateTime::parse(&row.get::<_, String>(2)?, &Rfc3339).unwrap(),
            provider_id: row.get::<_, String>(3)?,
            provider_type: ProviderType::from_str(&row.get::<_, String>(4)?).unwrap(),
            url: row.get(5)?,
            size,
            blob_format,
        })
    }
}

fn blob_format_from_sql(value: Option<String>) -> Option<BlobFormat> {
    match value {
        Some(string) => match string.as_str() {
            "Raw" => Some(BlobFormat::Raw),
            "HashSeq" => Some(BlobFormat::HashSeq),
            _ => None,
        },
        None => None,
    }
}
