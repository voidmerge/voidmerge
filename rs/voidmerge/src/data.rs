//! VoidMerge data types.

use crate::crypto::CryptoSigner;
use crate::types::{Hash, Value};
use bytes::Bytes;
use std::collections::HashMap;
use std::io::Result;
use std::sync::Arc;

/// Get a current time timestamp.
pub fn now() -> f64 {
    std::time::SystemTime::UNIX_EPOCH
        .elapsed()
        .expect("time error")
        .as_secs_f64()
}

#[inline(always)]
fn str_empty(s: &Arc<str>) -> bool {
    s.is_empty()
}

#[inline(always)]
fn f64_zero(f: &f64) -> bool {
    *f == 0.0
}

/// Base type representing all VoidMerge data.
#[derive(Default, Debug, serde::Serialize, serde::Deserialize)]
pub struct VmData {
    /// A type hint for this data type. This should be small to prevent adding
    /// serialization overhead, and should not start with "sys" because those
    /// are reserved for system types.
    #[serde(rename = "t", default, skip_serializing_if = "str_empty")]
    pub typ: Arc<str>,

    /// The identity of this data type instance. If this ident is empty,
    /// canonically VoidMerge will instead use the short hash.
    #[serde(rename = "i", default, skip_serializing_if = "Hash::is_empty")]
    pub ident: Hash,

    /// Microsecond created at timestamp for determining overwrite.
    #[serde(rename = "c", default, skip_serializing_if = "f64_zero")]
    pub created_secs: f64,

    /// Microsecond expires at timestamp for ttl.
    #[serde(rename = "e", default, skip_serializing_if = "f64_zero")]
    pub expires_secs: f64,

    /// Type specific or user/application additional data.
    #[serde(rename = "a", default, skip_serializing_if = "HashMap::is_empty")]
    pub app_data: HashMap<Arc<str>, Value>,

    /// Ident of the sysuser entry that has signed this data instance (if any).
    #[serde(rename = "s", default, skip_serializing_if = "Hash::is_empty")]
    pub signer: Hash,
}

impl VmData {
    /// Encode, hash, and sign this [VmData].
    pub fn sign(self, sign: &CryptoSigner) -> Result<VmDataSigned> {
        let data = crate::types::encode(&self)?;
        let sha512 = Hash::sha2_512(&data[..]);
        let signature = sign.sign_prehashed_512_bits(&sha512)?;
        Ok(VmDataSigned {
            parsed: self,
            sha512,
            data,
            signature,
        })
    }
}

/// Signed VoidMerge data.
#[derive(Default, Debug, serde::Serialize)]
pub struct VmDataSigned {
    /// The parsed [VmData].
    #[serde(skip_serializing)]
    pub parsed: VmData,

    /// The sha512 hash of the encoded bytes.
    #[serde(skip_serializing)]
    pub sha512: Hash,

    /// The encoded [VmData].
    #[serde(rename = "d", skip_serializing_if = "bytes::Bytes::is_empty")]
    pub data: Bytes,

    /// The cryptographic signature.
    #[serde(rename = "s", skip_serializing_if = "Hash::is_empty")]
    pub signature: Hash,
}

impl<'de> serde::Deserialize<'de> for VmDataSigned {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(serde::Deserialize)]
        struct Part {
            #[serde(default)]
            d: Bytes,
            #[serde(default)]
            s: Hash,
        }

        let part: Part = serde::Deserialize::deserialize(deserializer)?;

        let parsed: VmData =
            crate::types::decode(&part.d).map_err(serde::de::Error::custom)?;
        let sha512 = Hash::sha2_512(&part.d);

        Ok(Self {
            parsed,
            sha512,
            data: part.d,
            signature: part.s,
        })
    }
}

impl std::ops::Deref for VmDataSigned {
    type Target = VmData;

    fn deref(&self) -> &Self::Target {
        &self.parsed
    }
}

impl VmDataSigned {
    /// Get the canonical ident for this data.
    ///
    /// Either specified by the data itself,
    /// or the truncated (24 byte) sha512 hash.
    pub fn canon_ident(&self) -> Hash {
        if self.ident.is_empty() {
            self.short()
        } else {
            self.ident.clone()
        }
    }

    /// Get the short hash identifier for this data.
    pub fn short(&self) -> Hash {
        self.sha512.truncated(24)
    }
}
