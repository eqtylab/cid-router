pub mod auth;
pub mod cid_filter;
pub mod context;
pub mod crp;
pub mod db;
pub mod routes;

pub use context::Context;

// Some select multihash codes and multicodec codec codes

pub mod multihash {
    pub const SHA1: u64 = 0x11;
    pub const SHA256: u64 = 0x12;
    pub const BLAKE3: u64 = 0x1e;
}

pub mod multicodec {
    pub const RAW: u64 = 0x55;
    pub const DAG_CBOR: u64 = 0x71;
    pub const GIT_RAW: u64 = 0x78;
    pub const BLAKE3_HASHSEQ: u64 = 0x80;
}
