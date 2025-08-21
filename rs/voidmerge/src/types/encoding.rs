use super::*;
use std::collections::HashMap;

/// Encode into canonical msgpack format.
pub fn encode<T>(t: &T) -> Result<Bytes>
where
    T: serde::Serialize + ?Sized,
{
    use bytes::BufMut;

    let mut out = bytes::BytesMut::new().writer();

    rmp_serde::encode::write_named(&mut out, t)
        .map_err(std::io::Error::other)?;

    Ok(out.into_inner().freeze())
}

/// Decode from canonical msgpack format.
pub fn decode<T>(b: &[u8]) -> Result<T>
where
    T: serde::de::DeserializeOwned,
{
    rmp_serde::from_slice(b).map_err(std::io::Error::other)
}

/// Requesting an auth challenge.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthChalReq {
    /// The api token to include in the 'Authorization' header.
    pub token: Hash,

    /// The nonce to sign.
    pub nonce: Hash,
}

/// Responding to an auth challenge.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthChalRes {
    /// The signature over the provided nonce.
    pub nonce_sig: Vec<VmSignature>,

    /// Request access to the following contexts.
    /// `(<ContextHash>, <AppAuthData>)`.
    pub context_access: Vec<(Hash, Value)>,
}

/// A VoidMerge p2p message.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VmMsg {
    /// The context of this message.
    pub ctx: Hash,

    /// The addressable peer-hash of this message.
    pub peer: Hash,

    /// The payload of this message.
    pub data: Bytes,
}

/// An encoded select request.
#[derive(
    Clone, Debug, Default, serde::Serialize, serde::Deserialize, PartialEq,
)]
#[serde(rename_all = "camelCase")]
pub struct VmSelect {
    /// By default, select will return items of all types.
    /// If you would like to limit this, specify a list of types to include.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filter_by_types: Option<Vec<Arc<str>>>,

    /// If you would like to only return items with specific idents,
    /// specify that list of idents here.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filter_by_idents: Option<Vec<Hash>>,

    /// If you would like to only return items with specific short hashes,
    /// specify that list of hashes here.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filter_by_shorts: Option<Vec<Hash>>,

    /// By default, select results do not include per-item sizes.
    /// If you set returnSize to true, these sizes will be included.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub return_size: Option<bool>,

    /// By default, select results do not include the short hash.
    /// If you set returnShort to true, these hashes will be included.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub return_short: Option<bool>,

    /// By default, select results do not include the ident.
    /// If you set returnIdent to true, these idents will be included.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub return_ident: Option<bool>,

    /// By default, select results do not include the type of the item.
    /// If you set returnType to true, the type will be included with the items.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub return_type: Option<bool>,

    /// By default, select results do not include the actualy data content.
    /// If you set returnData to true, this will be included in the result.
    /// Note that this may result in a very large response that may get
    /// truncated. Instead, you could fetch a list of short hashes, and
    /// then make separate individual requests for the content data.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub return_data: Option<bool>,
}

/// An encoded select response item.
#[derive(
    Clone, Debug, Default, serde::Serialize, serde::Deserialize, PartialEq,
)]
#[serde(rename_all = "camelCase")]
pub struct VmSelectResponseItem {
    /// The type of this item, if requested.
    #[serde(default, rename = "type", skip_serializing_if = "Option::is_none")]
    pub type_: Option<Arc<str>>,

    /// The size in byte of this item, if requested.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<f64>,

    /// The short hash of this item, if requested.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub short: Option<Hash>,

    /// The ident of this item, if requested.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ident: Option<Hash>,

    /// The content data of this item, if requested.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Arc<VmObjSigned>>,
}

/// An encoded select response.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct VmSelectResponse {
    /// The number of active items which match the filters.
    pub count: f64,

    /// The sum of item sizes (in bytes) matching this select query.
    pub size: f64,

    /// The specific details if requested for the matching items.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub results: Vec<VmSelectResponseItem>,
}

/// VoidMerge encoded, parsed, bundled, hashed, and signed [VmObj].
#[derive(Clone, Debug, serde::Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct VmObjSigned {
    /// The parsed [VmObj].
    #[serde(skip_serializing)]
    pub parsed: VmObj,

    /// The encoded [VmObj].
    pub enc: Bytes,

    /// The sha512 hash of the encoded bytes.
    pub sha512: Hash,

    /// A list of signatures over the sha512 hash of the bytes.
    pub sigs: Vec<VmSignature>,
}

impl<'de> serde::Deserialize<'de> for VmObjSigned {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(serde::Deserialize)]
        struct Part {
            enc: Bytes,
            sha512: Hash,
            sigs: Vec<VmSignature>,
        }

        let d: Part = serde::Deserialize::deserialize(deserializer)?;

        let parsed: VmObj = decode(&d.enc).map_err(serde::de::Error::custom)?;

        Ok(Self {
            parsed,
            enc: d.enc,
            sha512: d.sha512,
            sigs: d.sigs,
        })
    }
}

impl std::ops::Deref for VmObjSigned {
    type Target = VmObj;

    fn deref(&self) -> &Self::Target {
        &self.parsed
    }
}

impl VmObjSigned {
    /// Get the canonical ident for this bundle.
    ///
    /// Either specified by the bundle itself,
    /// or the truncated (24 byte) sha512 hash.
    pub fn canon_ident(&self) -> Hash {
        self.ident
            .clone()
            .unwrap_or_else(|| self.sha512.truncated(24))
    }

    /// Get the short hash identifier of this bundle.
    pub fn short(&self) -> Hash {
        self.sha512.truncated(24)
    }
}

/// VoidMerge unified single encoding type.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct VmObj {
    /// The data type name.
    #[serde(rename = "type")]
    pub type_: Arc<str>,

    /// Identifier.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ident: Option<Hash>,

    /// Required dependencies for validation.
    #[serde(skip_serializing_if = "skip_deps")]
    pub deps: Option<Vec<Hash>>,

    /// Expiration seconds since unix epoch.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ttl_s: Option<f64>,

    /// App-defined payload.
    #[serde(skip_serializing_if = "skip_app")]
    pub app: Option<Value>,
}

fn skip_deps(deps: &Option<Vec<Hash>>) -> bool {
    match deps {
        None => true,
        Some(v) => v.is_empty(),
    }
}

fn skip_app(app: &Option<Value>) -> bool {
    matches!(app, None | Some(Value::Unit))
}

impl VmObj {
    /// Encode, hash, and sign this [VmObj].
    pub fn sign(self, sign: &MultiSign) -> Result<VmObjSigned> {
        let enc = encode(&self)?;
        let sha512 = Hash::sha2_512(&enc[..]);
        let sigs = sign.sign(&sha512);
        Ok(VmObjSigned {
            parsed: self,
            enc,
            sha512,
            sigs,
        })
    }
}

/// A cryptographic signature over [VmObj] data.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct VmSignature {
    /// The signing pubkey.
    pub pk: SignPublicKey,

    /// The signature material itself.
    pub sig: Signature,
}

/// VoidMerge logic code bundle.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum VmLogic {
    /// A single block of utf-8 code.
    #[serde(rename_all = "camelCase")]
    Utf8Single {
        /// All the utf8 code as a single block.
        code: Arc<str>,
    },
}

/// VoidMerge environment bundle.
#[derive(
    Clone, Debug, Default, serde::Serialize, serde::Deserialize, PartialEq,
)]
#[serde(rename_all = "camelCase")]
pub struct VmEnv {
    /// Public env data that will be available in the
    /// unauthenticated context status call.
    pub public: VmEnvPublic,

    /// Private env data that will only be available once authenticated.
    pub private: VmEnvPrivate,
}

/// VoidMerge public environment bundle.
#[derive(
    Clone, Debug, Default, serde::Serialize, serde::Deserialize, PartialEq,
)]
#[serde(rename_all = "camelCase")]
pub struct VmEnvPublic {
    /// List of well-known trusted server node urls in this cluster.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub servers: Vec<String>,

    /// App-specific additional data.
    #[serde(flatten)]
    pub app: HashMap<Arc<str>, Box<Value>>,
}

/// VoidMerge private environment bundle.
#[derive(
    Clone, Debug, Default, serde::Serialize, serde::Deserialize, PartialEq,
)]
#[serde(rename_all = "camelCase")]
pub struct VmEnvPrivate {
    /// Pubkeys authorized as context admins.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ctxadmin_pubkeys: Vec<Vec<SignPublicKey>>,

    /// App-specific additional data.
    #[serde(flatten)]
    pub app: HashMap<Arc<str>, Box<Value>>,
}
