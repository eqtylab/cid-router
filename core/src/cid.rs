use cid::Cid;
use iroh_blobs::Hash;
use multihash::Multihash;

pub mod mh_codes {
    pub const SHA1: u64 = 0x11;
    pub const SHA256: u64 = 0x12;
    pub const BLAKE3: u64 = 0x1e;
}

// TODO - make these not public, use Codec enum instead
pub mod mc_codes {
    pub const RAW: u64 = 0x55;
    pub const DAG_CBOR: u64 = 0x71;
    pub const GIT_RAW: u64 = 0x78;
    pub const BLAKE3_HASHSEQ: u64 = 0x80;
}

pub enum Codec {
    Raw,
    DagCbor,
    GitRaw,
    Blake3HashSeq,
}

impl Codec {
    fn code(&self) -> u64 {
        match self {
            Codec::Raw => mc_codes::RAW,
            Codec::DagCbor => mc_codes::DAG_CBOR,
            Codec::GitRaw => mc_codes::GIT_RAW,
            Codec::Blake3HashSeq => mc_codes::BLAKE3_HASHSEQ,
        }
    }
}

pub fn blake3_hash_to_cid(hash: Hash, codec: Codec) -> Cid {
    let mh = Multihash::wrap(crate::cid::mh_codes::BLAKE3, hash.as_bytes()).unwrap();
    Cid::new_v1(codec.code(), mh)
}
