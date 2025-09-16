use std::ops::{BitAnd, BitOr, Not};

use cid::Cid;
use iroh_blobs::Hash;
use serde::{Deserialize, Serialize};

const RAW_CODE_POINT: u64 = 0x55;
const BLAKE_3_CODE_POINT: u64 = 0x1e;

pub fn blake3_hash_to_cid(hash: Hash) -> Cid {
    let mh = multihash::Multihash::wrap(BLAKE_3_CODE_POINT, hash.as_bytes()).unwrap();
    Cid::new_v1(RAW_CODE_POINT, mh)
}

/// CID Filter
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CidFilter {
    None,
    MultihashCodeFilter(CodeFilter<u64>),
    CodecFilter(CodeFilter<u64>),
    And(Vec<Self>),
    Or(Vec<Self>),
    Not(Box<Self>),
}

/// Code Filter
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CodeFilter<T> {
    Eq(T),
    Gt(T),
    Lt(T),
    And(Vec<Self>),
    Or(Vec<Self>),
    Not(Box<Self>),
}

impl CidFilter {
    /// Check if a CID matches the CID filter
    pub fn is_match(&self, cid: &Cid) -> bool {
        match self {
            Self::None => true,
            Self::MultihashCodeFilter(f) => f.is_match(cid.hash().code()),
            Self::CodecFilter(f) => f.is_match(cid.codec()),
            Self::And(fs) => fs.iter().all(|f| f.is_match(cid)),
            Self::Or(fs) => fs.iter().any(|f| f.is_match(cid)),
            Self::Not(ref f) => !f.is_match(cid),
        }
    }
}

impl<T> CodeFilter<T>
where
    T: Copy + PartialEq + PartialOrd,
{
    /// Check if a code matches the code filter
    pub fn is_match(&self, value: T) -> bool {
        match self {
            Self::Eq(eq_val) => value == *eq_val,
            Self::Gt(gt_val) => value > *gt_val,
            Self::Lt(lt_val) => value < *lt_val,
            Self::And(fs) => fs.iter().all(|f| f.is_match(value)),
            Self::Or(fs) => fs.iter().any(|f| f.is_match(value)),
            Self::Not(ref f) => !f.is_match(value),
        }
    }
}

impl BitAnd for CidFilter {
    type Output = Self;

    fn bitand(self, rhs: Self) -> Self {
        Self::And(vec![self, rhs])
    }
}

impl BitOr for CidFilter {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self {
        Self::Or(vec![self, rhs])
    }
}

impl Not for CidFilter {
    type Output = Self;

    fn not(self) -> Self {
        Self::Not(Box::new(self))
    }
}

impl BitAnd for CodeFilter<u64> {
    type Output = Self;

    fn bitand(self, rhs: Self) -> Self {
        Self::And(vec![self, rhs])
    }
}

impl BitOr for CodeFilter<u64> {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self {
        Self::Or(vec![self, rhs])
    }
}

impl Not for CodeFilter<u64> {
    type Output = Self;

    fn not(self) -> Self {
        Self::Not(Box::new(self))
    }
}

#[cfg(test)]
mod tests {
    use cid::multihash::Multihash;

    use super::*;
    use crate::{
        multicodec::{DAG_CBOR, RAW},
        multihash::{BLAKE3, SHA256},
    };

    fn blake3_raw() -> Cid {
        Cid::new_v1(0x55, Multihash::wrap(0x1e, &[0; 32]).unwrap())
    }

    fn sha256_raw() -> Cid {
        Cid::new_v1(0x55, Multihash::wrap(0x12, &[0; 32]).unwrap())
    }

    fn sha256_dag_cbor() -> Cid {
        Cid::new_v1(0x71, Multihash::wrap(0x12, &[0; 32]).unwrap())
    }

    #[test]
    fn any() {
        let filter = CidFilter::None;

        assert!(filter.is_match(&blake3_raw()));
        assert!(filter.is_match(&sha256_raw()));
        assert!(filter.is_match(&sha256_dag_cbor()));
    }

    #[test]
    fn multihash_eq() {
        let filter = CidFilter::MultihashCodeFilter(CodeFilter::Eq(BLAKE3));

        assert!(filter.is_match(&blake3_raw()));
        assert!(!filter.is_match(&sha256_raw()));
        assert!(!filter.is_match(&sha256_dag_cbor()));
    }

    #[test]
    fn codec_eq() {
        let filter = CidFilter::CodecFilter(CodeFilter::Eq(RAW));

        assert!(filter.is_match(&blake3_raw()));
        assert!(filter.is_match(&sha256_raw()));
        assert!(!filter.is_match(&sha256_dag_cbor()));
    }

    #[test]
    fn and() {
        let filter = CidFilter::MultihashCodeFilter(CodeFilter::Eq(SHA256))
            & CidFilter::CodecFilter(CodeFilter::Eq(DAG_CBOR));

        assert!(!filter.is_match(&blake3_raw()));
        assert!(!filter.is_match(&sha256_raw()));
        assert!(filter.is_match(&sha256_dag_cbor()));
    }

    #[test]
    fn or() {
        let filter = CidFilter::MultihashCodeFilter(CodeFilter::Eq(SHA256))
            | CidFilter::CodecFilter(CodeFilter::Eq(DAG_CBOR));

        assert!(!filter.is_match(&blake3_raw()));
        assert!(filter.is_match(&sha256_raw()));
        assert!(filter.is_match(&sha256_dag_cbor()));
    }

    #[test]
    fn not() {
        let filter = !CidFilter::CodecFilter(CodeFilter::Eq(RAW));

        assert!(!filter.is_match(&blake3_raw()));
        assert!(!filter.is_match(&sha256_raw()));
        assert!(filter.is_match(&sha256_dag_cbor()));
    }
}
