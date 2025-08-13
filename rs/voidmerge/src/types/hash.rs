use super::*;

/// A hash type.
#[derive(
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    serde::Serialize,
    serde::Deserialize,
)]
pub struct Hash(Bytes);

impl Default for Hash {
    fn default() -> Self {
        Self(Bytes::from_static(b""))
    }
}

impl std::fmt::Display for Hash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use base64::prelude::*;

        f.write_str(&BASE64_URL_SAFE_NO_PAD.encode(&self.0))
    }
}

impl std::fmt::Debug for Hash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use base64::prelude::*;
        f.write_str("\"")?;
        f.write_str(&BASE64_URL_SAFE_NO_PAD.encode(&self.0))?;
        f.write_str("\"")
    }
}

impl std::str::FromStr for Hash {
    type Err = std::io::Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        use base64::prelude::*;

        BASE64_URL_SAFE_NO_PAD
            .decode(s)
            .map_err(std::io::Error::other)
            .map(|u| Hash::from(&u[..]))
    }
}

impl From<Bytes> for Hash {
    fn from(b: Bytes) -> Self {
        Self(b)
    }
}

impl From<&[u8]> for Hash {
    fn from(r: &[u8]) -> Self {
        Self(Bytes::copy_from_slice(r))
    }
}

impl From<Hash> for Bytes {
    fn from(h: Hash) -> Self {
        h.0
    }
}

impl std::ops::Deref for Hash {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Hash {
    /// Hash that can be written as a const constructor.
    pub const fn from_static(input: &'static [u8]) -> Self {
        Self(Bytes::from_static(input))
    }

    /// Returns true if the hash buffer is zero length.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Generate cryptographically secure randomized bytes.
    pub fn rand(len: usize) -> Self {
        use rand::prelude::*;
        let mut m = bytes::BytesMut::zeroed(len);
        rand::thread_rng().fill(&mut m[..]);
        m.freeze().into()
    }

    /// Generate nonce bytes. (Shortcut for rand(24)).
    pub fn nonce() -> Self {
        Self::rand(24)
    }

    /// Generate a sha2-512 hash over given bytes.
    pub fn sha2_512(input: &[u8]) -> Self {
        use sha2::*;
        Sha512::digest(input)[..].into()
    }

    /// Get a truncated version of this hash. (Commonly 24 bytes).
    pub fn truncated(&self, len: usize) -> Self {
        if self.len() == len {
            return self.clone();
        }

        Self(self.0.slice(0..len))
    }
}
