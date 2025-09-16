use cid::Cid;
use iroh_blobs::Hash;
use multihash::Multihash;

pub mod mh_codes {
    pub const SHA1: u64 = 0x11;
    pub const SHA256: u64 = 0x12;
    pub const BLAKE3: u64 = 0x1e;
}

pub mod mc_codes {
    pub const RAW: u64 = 0x55;
    pub const DAG_CBOR: u64 = 0x71;
    pub const GIT_RAW: u64 = 0x78;
    pub const BLAKE3_HASHSEQ: u64 = 0x80;
}

pub fn blake3_hash_to_cid(hash: Hash) -> Cid {
    let mh = Multihash::wrap(crate::cid::mh_codes::BLAKE3, hash.as_bytes()).unwrap();
    Cid::new_v1(crate::cid::mc_codes::RAW, mh)
}
