pub mod external;
pub mod ipfs;
pub mod iroh;

use anyhow::Result;
use async_trait::async_trait;
use cid::{multihash::Multihash, Cid};
use cid_filter::CidFilter;
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
