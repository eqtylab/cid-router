pub mod external;
pub mod ipfs;
pub mod iroh;

use std::pin::Pin;

use anyhow::Result;
use async_trait::async_trait;
use cid::{multihash::Multihash, Cid};
use cid_filter::CidFilter;
use futures::Stream;
use routes::Route;
use serde_json::Value;
use sha2::{Digest, Sha256};

/// CID Route Provider (CRP) Trait
#[async_trait]
pub trait Crp {
    fn cid_filter(&self) -> CidFilter;

    async fn get_routes_for_cid(&self, cid: &Cid) -> Result<Vec<Route>>;

    fn provider_config(&self) -> Value;

    fn provider_is_eligible_for_cid(&self, cid: &Cid) -> bool {
        self.cid_filter().is_match(cid)
    }

    fn provider_id(&self) -> String {
        // provider ID is the JCS CID of its config
        let jcs = serde_jcs::to_string(&self.provider_config())
            .expect("unexpectedly failed to serialize a config type");
        let sha256 = {
            let mut hasher = Sha256::new();
            hasher.update(jcs.as_bytes());
            hasher.finalize()
        };
        let multihash =
            Multihash::wrap(0x12, &sha256).expect("unexpectedly failed to wrap a multihash");

        Cid::new_v1(0xb601, multihash).to_string()
    }
}

/// A Resolver can dereference a CID pointer, turning it into a stream of bytes, accepting
/// authentication data.
#[async_trait]
pub trait Resolver {
    async fn get(
        &self,
        cid: &Cid,
        auth: Vec<u8>,
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

/// SizeResolver returns the length in bytes of the blob a CID points at.
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
