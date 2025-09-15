//! Cryptography types.

use std::io::Result;
use std::sync::Arc;

/// Signature material.
pub type CryptoSignature = crate::types::Hash;

/// Signing public material.
pub type CryptoSignPublic = crate::types::Hash;

#[derive(zeroize::Zeroize, zeroize::ZeroizeOnDrop)]
struct CryptoSignSecretInner(Vec<u8>);

/// Signing secret material.
#[derive(Clone)]
pub struct CryptoSignSecret(Arc<CryptoSignSecretInner>);

impl From<Vec<u8>> for CryptoSignSecret {
    fn from(f: Vec<u8>) -> Self {
        Self(Arc::new(CryptoSignSecretInner(f)))
    }
}

impl std::ops::Deref for CryptoSignSecret {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        &(self.0).0
    }
}

impl std::fmt::Debug for CryptoSignSecret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CryptoSignSecret").finish()
    }
}

/// Cryptograhic signatures.
pub trait CryptoSign: std::fmt::Debug + 'static + Send + Sync {
    /// Algorithm identifier.
    fn alg(&self) -> &'static str;

    /// Generate a new signing keypair.
    fn generate(&self) -> Result<(CryptoSignPublic, CryptoSignSecret)>;

    /// Sign prehashed data. The data we are signing should be a 512 bit hash.
    fn sign_prehashed_512_bits(
        &self,
        sk: &CryptoSignSecret,
        hash: &[u8],
    ) -> Result<CryptoSignature>;

    /// Verify prehashed data. The data to verify should be a 512 bit hash.
    fn verify_prehashed_512_bits(
        &self,
        pk: &CryptoSignPublic,
        signature: &CryptoSignature,
        hash: &[u8],
    ) -> Result<()>;
}

/// Dyn type [CryptoSign].
pub type DynCryptoSign = Arc<dyn CryptoSign + 'static + Send + Sync>;

struct Digest<'lt>(&'lt [u8]);

impl<'lt> digest::crypto_common::OutputSizeUser for Digest<'lt> {
    type OutputSize = digest::typenum::U64;
}

impl<'lt> digest::Digest for Digest<'lt> {
    fn new() -> Self {
        unimplemented!()
    }

    fn new_with_prefix(_data: impl AsRef<[u8]>) -> Self {
        unimplemented!()
    }

    fn update(&mut self, _data: impl AsRef<[u8]>) {
        unimplemented!()
    }

    fn chain_update(self, _data: impl AsRef<[u8]>) -> Self {
        unimplemented!()
    }

    fn finalize(
        self,
    ) -> digest::generic_array::GenericArray<u8, Self::OutputSize> {
        digest::generic_array::GenericArray::clone_from_slice(self.0)
    }

    fn finalize_into(
        self,
        _out: &mut digest::generic_array::GenericArray<u8, Self::OutputSize>,
    ) {
        unimplemented!()
    }

    fn finalize_reset(
        &mut self,
    ) -> digest::generic_array::GenericArray<u8, Self::OutputSize>
    where
        Self: digest::FixedOutputReset,
    {
        unimplemented!()
    }

    fn finalize_into_reset(
        &mut self,
        _out: &mut digest::generic_array::GenericArray<u8, Self::OutputSize>,
    ) where
        Self: digest::FixedOutputReset,
    {
        unimplemented!()
    }

    fn reset(&mut self)
    where
        Self: digest::Reset,
    {
        unimplemented!()
    }

    fn output_size() -> usize {
        unimplemented!()
    }

    fn digest(
        _data: impl AsRef<[u8]>,
    ) -> digest::generic_array::GenericArray<u8, Self::OutputSize> {
        unimplemented!()
    }
}

#[cfg(feature = "ml_dsa")]
mod ml_dsa;

#[cfg(feature = "ml_dsa")]
pub use ml_dsa::*;

#[cfg(feature = "p256")]
mod p256;

#[cfg(feature = "p256")]
pub use p256::*;

#[cfg(feature = "ed25519")]
mod ed25519;

#[cfg(feature = "ed25519")]
pub use ed25519::*;
