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
    pub fn alg_list(&self) -> impl Iterator<Item = &'static str> {
        self.0.keys().copied()
    }

    /// Get a crypto instance by algorithm.
    pub fn crypto(&self, alg: &str) -> Option<&DynCryptoSign> {
        self.0.get(alg)
    }
}

impl Default for CryptoSignRegistry {
    fn default() -> Self {
        let algs: Vec<DynCryptoSign> = vec![
            #[cfg(feature = "p256")]
            Arc::new(
                CryptoSignP256::new()
                    .expect("failed to construct CryptoSignP256 instance"),
            ),
            #[cfg(feature = "ed25519")]
            Arc::new(
                CryptoSignEd25519::new()
                    .expect("failed to construct CryptoSignEd25519 instance"),
            ),
            #[cfg(feature = "ml_dsa")]
            Arc::new(
                CryptoSignMlDsa44::new()
                    .expect("failed to construct CryptoSignMlDsa44 instance"),
            ),
            #[cfg(feature = "ml_dsa")]
            Arc::new(
                CryptoSignMlDsa65::new()
                    .expect("failed to construct CryptoSignMlDsa65 instance"),
            ),
            #[cfg(feature = "ml_dsa")]
            Arc::new(
                CryptoSignMlDsa87::new()
                    .expect("failed to construct CryptoSignMlDsa87 instance"),
            ),
        ];

        CryptoSignRegistry::new(algs)
    }
}

struct VerifierItem {
    pub sign: DynCryptoSign,
    pub pk: CryptoSignPublic,
}

/// A verifier capable of verifying VoidMerge signatures.
#[derive(Clone)]
pub struct CryptoVerifier {
    verify_list: Arc<[VerifierItem]>,
}

impl CryptoVerifier {
    /// Construct a new [CryptoVerifier] from a sysuser data item.
    pub fn with_sysuser(
        registry: &CryptoSignRegistry,
        sysuser: &crate::data::VmDataSigned,
    ) -> Result<Self> {
        #[derive(Debug, serde::Deserialize)]
        struct Pk {
            a: String,
            p: CryptoSignPublic,
        }

        let syspk = match sysuser.app_data.get("syspk") {
            Some(crate::types::Value::Bytes(syspk)) => syspk,
            _ => {
                return Err(std::io::Error::other(
                    "invalid sysuser (no syspk app data)",
                ));
            }
        };

        let syspk: Vec<Pk> = crate::types::decode(syspk)?;

        let mut verify_list = Vec::new();

        for Pk { a, p } in syspk {
            let sign = registry.crypto(&a).ok_or_else(|| {
                std::io::Error::other(format!("invalid signing algorithm: {a}"))
            })?;
            verify_list.push(VerifierItem {
                sign: sign.clone(),
                pk: p,
            });
        }

        let this = Self {
            verify_list: verify_list.into_boxed_slice().into(),
        };

        this.verify_prehashed_512_bits(&sysuser.signature, &sysuser.sha512)?;

        Ok(this)
    }

    /// Verify prehashed data. The data to verify should be a 512 bit hash.
    fn verify_prehashed_512_bits(
        &self,
        signature: &CryptoSignature,
        hash: &[u8],
    ) -> Result<()> {
        let sigs: Vec<CryptoSignature> = crate::types::decode(signature)?;
        if sigs.is_empty() || sigs.len() != self.verify_list.len() {
            return Err(std::io::Error::other("signature count mismatch"));
        }
        for (sig, item) in sigs.into_iter().zip(self.verify_list.iter()) {
            item.sign.verify_prehashed_512_bits(&item.pk, &sig, hash)?;
        }
        Ok(())
    }
}

struct SignerItem {
    pub sign: DynCryptoSign,
    pub sk: CryptoSignSecret,
    #[allow(dead_code)]
    pub pk: CryptoSignPublic,
}

/// A signer capable of generating VoidMerge signatures.
pub struct CryptoSigner {
    sign_list: Arc<[SignerItem]>,
    sysuser: crate::data::VmDataSigned,
    verifier: CryptoVerifier,
}

impl CryptoSigner {
    /// Generate a new signer instance.
    pub fn generate(
        alg_list: impl IntoIterator<Item = DynCryptoSign>,
    ) -> Result<Self> {
        use crate::types::Value;

        let mut sign_list = Vec::new();
        let mut verify_list = Vec::new();
        let mut syspk = Value::array_new();

        for sign in alg_list {
            let (pk, sk) = sign.generate()?;
            let mut pkdata = Value::map_new();
            pkdata.map_insert("a".into(), sign.alg().into());
            pkdata.map_insert("p".into(), pk.clone().into());
            syspk.array_push(pkdata);
            verify_list.push(VerifierItem {
                sign: sign.clone(),
                pk: pk.clone(),
            });
            sign_list.push(SignerItem { sign, sk, pk });
        }

        let syspk = crate::types::encode(&syspk)?;
        let ident = crate::types::Hash::sha2_512(&syspk).truncated(24);
        let signer = ident.clone();

        let mut data = crate::data::VmData {
            typ: "sysuser".into(),
            ident,
            created_secs: crate::data::now(),
            signer,
            ..Default::default()
        };

        data.app_data.insert("syspk".into(), syspk.into());

        let verifier = CryptoVerifier {
            verify_list: verify_list.into_boxed_slice().into(),
        };

        let mut this = Self {
            sign_list: sign_list.into_boxed_slice().into(),
            sysuser: Default::default(),
            verifier,
        };

        let pk = data.sign(&this)?;

        this.sysuser = pk;

        Ok(this)
    }

    /// Load a signer up from a registry and encoded secrets.
    pub fn with_secrets(
        registry: &CryptoSignRegistry,
        secrets: &[u8],
    ) -> Result<Self> {
        #[derive(Debug, serde::Deserialize)]
        struct Key {
            a: String,
            p: crate::types::Hash,
            s: Vec<u8>,
        }
        #[derive(Debug, serde::Deserialize)]
        struct Sec {
            u: crate::data::VmDataSigned,
            k: Vec<Key>,
        }
        let sec: Sec = crate::types::decode(secrets)?;
        let Sec { u, k } = sec;

        let mut sign_list = Vec::with_capacity(k.len());

        for Key { a, p, s } in k {
            let sign = registry.crypto(&a).ok_or_else(|| {
                std::io::Error::other(format!("invalid signing algorithm: {a}"))
            })?;
            sign_list.push(SignerItem {
                sign: sign.clone(),
                sk: s.into(),
                pk: p,
            });
        }

        let verifier = CryptoVerifier::with_sysuser(registry, &u)?;

        Ok(Self {
            sign_list: sign_list.into_boxed_slice().into(),
            sysuser: u,
            verifier,
        })
    }

    /// List the signing algorithms used by this signer.
    pub fn alg_list(&self) -> impl Iterator<Item = &'static str> {
        self.sign_list.iter().map(|item| item.sign.alg())
    }

    /// Get the sysuser associated with this signer.
    pub fn sysuser(&self) -> &crate::data::VmDataSigned {
        &self.sysuser
    }

    /// Get the crypto verifier for this signer.
    pub fn verifier(&self) -> &CryptoVerifier {
        &self.verifier
    }

    /// Encode secrets.
    pub fn encode_secrets(&self) -> Result<CryptoSignSecret> {
        #[derive(serde::Serialize)]
        struct Key<'lt> {
            a: &'static str,
            p: &'lt CryptoSignPublic,
            s: &'lt [u8],
        }

        #[derive(serde::Serialize)]
        struct Sec<'lt> {
            u: &'lt crate::data::VmDataSigned,
            k: Vec<Key<'lt>>,
        }

        let k = self
            .sign_list
            .iter()
            .map(|i| Key {
                a: i.sign.alg(),
                p: &i.pk,
                s: &i.sk,
            })
            .collect();

        let s = Sec {
            u: &self.sysuser,
            k,
        };

        encode_secret(&s)
    }

    /// Sign some data with our signer.
    pub fn sign_prehashed_512_bits(
        &self,
        hash: &[u8],
    ) -> Result<CryptoSignature> {
        use crate::types::Value;
        let mut sig = Value::array_new();
        for item in self.sign_list.iter() {
            sig.array_push(
                item.sign.sign_prehashed_512_bits(&item.sk, hash)?.into(),
            );
        }
        let sig = crate::types::encode(&sig)?;
        Ok(sig.into())
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    #[cfg(all(feature = "p256", feature = "ed25519"))]
    fn signer_basic() {
        let reg = CryptoSignRegistry::default();
        let sig = CryptoSigner::generate([
            reg.crypto("p256").unwrap().clone(),
            reg.crypto("ed25519").unwrap().clone(),
        ])
        .unwrap();
        let enc = sig.encode_secrets().unwrap();
        println!("secrets are {} bytes", enc.len());
        let sig = CryptoSigner::with_secrets(&reg, &enc).unwrap();
        println!("{:#?}", sig.sysuser());
        let enc = crate::types::encode(sig.sysuser()).unwrap();
        println!("encoded is {} bytes", enc.len());
    }
}
