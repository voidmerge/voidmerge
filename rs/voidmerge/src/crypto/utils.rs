//! Crypto utils.

use super::*;
use std::collections::HashMap;

/// Registry for available crypto signing algorithms.
#[derive(Debug, Clone)]
pub struct CryptoSignRegistry(Arc<HashMap<&'static str, DynCryptoSign>>);

impl CryptoSignRegistry {
    /// Construct a new registry instance.
    pub fn new(s: impl IntoIterator<Item = DynCryptoSign>) -> Self {
        Self(Arc::new(s.into_iter().map(|s| (s.alg(), s)).collect()))
    }

    /// List the available algorithms.
    pub fn alg_list(&self) -> impl Iterator<Item = &&'static str> {
        self.0.keys()
    }

    /// Get a crypto instance by algorithm.
    pub fn crypto(&self, alg: &str) -> Option<&DynCryptoSign> {
        self.0.get(alg)
    }
}

impl Default for CryptoSignRegistry {
    fn default() -> Self {
        let mut algs: Vec<DynCryptoSign> = Vec::new();

        #[cfg(feature = "p256")]
        algs.push(Arc::new(
            CryptoSignP256::new()
                .expect("failed to construct CryptoSignP256 instance"),
        ));

        #[cfg(feature = "ed25519")]
        algs.push(Arc::new(
            CryptoSignEd25519::new()
                .expect("failed to construct CryptoSignEd25519 instance"),
        ));

        #[cfg(feature = "ml_dsa")]
        algs.push(Arc::new(
            CryptoSignMlDsa44::new()
                .expect("failed to construct CryptoSignMlDsa44 instance"),
        ));

        #[cfg(feature = "ml_dsa")]
        algs.push(Arc::new(
            CryptoSignMlDsa65::new()
                .expect("failed to construct CryptoSignMlDsa65 instance"),
        ));

        #[cfg(feature = "ml_dsa")]
        algs.push(Arc::new(
            CryptoSignMlDsa87::new()
                .expect("failed to construct CryptoSignMlDsa87 instance"),
        ));

        CryptoSignRegistry::new(algs)
    }
}

struct SignerItem {
    pub sign: DynCryptoSign,
    pub sk: CryptoSignSecret,
    pub pk: CryptoSignPublic,
}

/// A signer capable of generating VoidMerge signatures.
pub struct CryptoSigner(Arc<[SignerItem]>);

impl CryptoSigner {
    /// Sign some data with our signer.
    pub fn sign_prehashed_512_bits(&self, hash: &[u8]) -> Result<CryptoSignature> {
        todo!()
    }
}
