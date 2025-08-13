use super::*;

/// Indicates a public key (`pub-` in base64url).
pub const B_PUB: &[u8] = &[166, 230, 254];

/// Indicates a secret key (`sec-` in base64url).
pub const B_SEC: &[u8] = &[177, 231, 62];

/// Indicates a signature (`sig-` in base64url).
pub const B_SIG: &[u8] = &[178, 40, 62];

/// A base sign type.
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
#[serde(transparent)]
pub struct SignType(Bytes);

impl From<Bytes> for SignType {
    fn from(b: Bytes) -> Self {
        Self(b)
    }
}

impl From<SignType> for Bytes {
    fn from(s: SignType) -> Self {
        s.0
    }
}

impl From<Hash> for SignType {
    fn from(h: Hash) -> Self {
        Self(h.into())
    }
}

impl From<SignType> for Hash {
    fn from(s: SignType) -> Self {
        s.0.into()
    }
}

impl std::fmt::Display for SignType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use base64::prelude::*;
        f.write_str(&BASE64_URL_SAFE_NO_PAD.encode(&self.0))
    }
}

impl std::fmt::Debug for SignType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use base64::prelude::*;
        if self.typ() == B_SEC {
            f.write_str(&BASE64_URL_SAFE_NO_PAD.encode(&self.0[..6]))?;
            f.write_str("<secret>")
        } else {
            f.write_str(&BASE64_URL_SAFE_NO_PAD.encode(&self.0))
        }
    }
}

impl std::str::FromStr for SignType {
    type Err = std::io::Error;

    fn from_str(s: &str) -> Result<Self> {
        use base64::prelude::*;
        Ok(Self(
            BASE64_URL_SAFE_NO_PAD
                .decode(s)
                .map_err(std::io::Error::other)?
                .into(),
        ))
    }
}

impl SignType {
    /// Construct a SignType from component parts.
    pub fn from_parts(
        alg: &'static [u8],
        type_: &'static [u8],
        material: &[u8],
    ) -> Self {
        let mut out = bytes::BytesMut::with_capacity(
            alg.len() + type_.len() + material.len(),
        );
        out.extend_from_slice(alg);
        out.extend_from_slice(type_);
        out.extend_from_slice(material);
        Self(out.freeze())
    }

    /// Get the algorithm this type is associated with.
    pub fn alg(&self) -> Arc<str> {
        use base64::prelude::*;
        BASE64_URL_SAFE_NO_PAD.encode(&self.0[..3]).into()
    }

    /// Check if this is a public key.
    pub fn is_public(&self) -> bool {
        self.typ() == B_PUB
    }

    /// Convert to a public key if this is a public key type.
    pub fn to_public(&self) -> Option<SignPublicKey> {
        if self.is_public() {
            Some(SignPublicKey(self.clone()))
        } else {
            None
        }
    }

    /// Check if this is a secret key.
    pub fn is_secret(&self) -> bool {
        self.typ() == B_SEC
    }

    /// Convert to a public key if this is a public key type.
    pub fn to_secret(&self) -> Option<SignSecretKey> {
        if self.is_secret() {
            Some(SignSecretKey(self.clone()))
        } else {
            None
        }
    }

    /// Check if this is a signature.
    pub fn is_signature(&self) -> bool {
        self.typ() == B_SIG
    }

    /// Convert to a public key if this is a public key type.
    pub fn to_signature(&self) -> Option<Signature> {
        if self.is_signature() {
            Some(Signature(self.clone()))
        } else {
            None
        }
    }

    /// Get the typ.
    pub fn typ(&self) -> &[u8] {
        &self.0[3..6]
    }

    /// Get the material for this sign type.
    pub fn material(&self) -> &[u8] {
        &self.0[6..]
    }
}

/// Signing public key.
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
#[serde(transparent)]
pub struct SignPublicKey(SignType);

/// Signing secret key.
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
pub struct SignSecretKey(SignType);

/// Signing signature.
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
#[serde(transparent)]
pub struct Signature(SignType);

macro_rules! imp {
    ($s:ty, $typ:expr) => {
        impl std::str::FromStr for $s {
            type Err = std::io::Error;

            fn from_str(s: &str) -> Result<Self> {
                let out: SignType = s.parse()?;
                if out.typ() != $typ {
                    return Err(std::io::Error::other("invalid sign typ"));
                }
                Ok(Self(out))
            }
        }

        impl std::fmt::Display for $s {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                self.0.fmt(f)
            }
        }

        impl std::fmt::Debug for $s {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                self.0.fmt(f)
            }
        }

        impl $s {
            /// Construct this sign type from parts.
            pub fn from_parts(alg: &'static [u8], material: &[u8]) -> Self {
                Self(SignType::from_parts(alg, $typ, material))
            }

            /// Get the algorimth this sign type is used with.
            pub fn alg(&self) -> Arc<str> {
                self.0.alg()
            }

            /// Get the material of this sign type.
            pub fn material(&self) -> &[u8] {
                self.0.material()
            }
        }
    };
}

imp!(SignPublicKey, B_PUB);
imp!(SignSecretKey, B_SEC);
imp!(Signature, B_SIG);

/// A cryptographic signature module.
pub trait ModuleSign: std::fmt::Debug + 'static + Send + Sync {
    /// The type of algorithm used for signing.
    fn alg(&self) -> &'static str;

    /// Create a new signing secret key.
    fn gen_secret(&self) -> SignSecretKey;

    /// Generate a public key from a secret key.
    fn gen_public(&self, secret: &SignSecretKey) -> SignPublicKey;

    /// Cryptographically sign some data.
    fn sign(&self, secret: &SignSecretKey, data: &[u8]) -> Signature;

    /// Cryptographically verify signed data.
    fn verify(&self, sig: &Signature, pk: &SignPublicKey, data: &[u8]) -> bool;
}

/// Trait object [ModuleSign].
pub type DynModuleSign = Arc<dyn ModuleSign + 'static + Send + Sync>;

#[derive(Clone, Debug)]
struct SignData {
    sign: DynModuleSign,
    sk: SignSecretKey,
    pk: SignPublicKey,
}

/// The multi-sign module manages multiple signatures.
#[derive(Debug)]
pub struct MultiSign {
    runtime_store: DynModuleRuntimeStore,
    sign: Mutex<Vec<SignData>>,
}

impl MultiSign {
    /// Construct a new [MultiSign] instance.
    pub fn new(runtime_store: DynModuleRuntimeStore) -> Self {
        Self {
            runtime_store,
            sign: Default::default(),
        }
    }

    /// Add a signing module to this [MultiSign] instance.
    pub async fn add_sign(&self, sign: DynModuleSign) -> Result<()> {
        let (sk, pk) = self.runtime_store.assert_sign_keypair(&sign).await?;
        self.sign.lock().unwrap().push(SignData { sign, sk, pk });
        Ok(())
    }

    /// Get the configured public keys.
    pub fn public_keys(&self) -> Vec<SignPublicKey> {
        self.sign
            .lock()
            .unwrap()
            .iter()
            .map(|SignData { pk, .. }| pk.clone())
            .collect()
    }

    /// Sign with the registered signature modules.
    pub fn sign(&self, data: &[u8]) -> Vec<VmSignature> {
        let all = self.sign.lock().unwrap().clone();
        all.into_iter()
            .map(|SignData { sign, sk, pk }| {
                let sig = sign.sign(&sk, data);
                VmSignature { pk, sig }
            })
            .collect()
    }

    /// Verify signatures.
    pub fn verify(&self, data: &[u8], sigs: &[VmSignature]) -> bool {
        use std::collections::HashMap;
        #[derive(Default)]
        struct ToDo {
            pk: Option<SignPublicKey>,
            sig: Option<Signature>,
            sign: Option<DynModuleSign>,
        }
        let mut map: HashMap<Arc<str>, ToDo> = HashMap::new();
        for sig in sigs.iter() {
            let r = map.entry(sig.sig.alg()).or_default();
            r.pk = Some(sig.pk.clone());
            r.sig = Some(sig.sig.clone());
        }
        let sign = self.sign.lock().unwrap().clone();
        for SignData { sign, .. } in sign {
            let alg = sign.alg().into();
            map.entry(alg).or_default().sign = Some(sign);
        }
        for (_, ToDo { pk, sig, sign }) in map {
            let pk = match pk {
                None => return false,
                Some(pk) => pk,
            };
            let sig = match sig {
                None => return false,
                Some(sig) => sig,
            };
            let sign = match sign {
                None => return false,
                Some(sign) => sign,
            };
            if !sign.verify(&sig, &pk, data) {
                return false;
            }
        }
        true
    }
}
