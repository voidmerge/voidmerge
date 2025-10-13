//! Bytes extension utilities.

use crate::*;
use ::bytes::Bytes;

/// Bytes extension utilities.
pub trait BytesExt {
    /// Build bytes from msgpack encoding a type.
    fn from_encode(t: &(impl serde::Serialize + ?Sized)) -> Result<Bytes>;

    /// From base64url.
    fn from_b64(s: &str) -> Result<Bytes>;

    /// Decode bytes into a type.
    fn to_decode<T>(&self) -> Result<T>
    where
        T: serde::de::DeserializeOwned;

    /// To base64url.
    fn to_b64(&self) -> String;
}

impl BytesExt for Bytes {
    fn from_encode(t: &(impl serde::Serialize + ?Sized)) -> Result<Bytes> {
        use bytes::BufMut;

        let mut out = bytes::BytesMut::new().writer();

        rmp_serde::encode::write_named(&mut out, t).map_err(Error::other)?;

        Ok(out.into_inner().freeze())
    }

    fn from_b64(s: &str) -> Result<Bytes> {
        use base64::prelude::*;
        let v = BASE64_URL_SAFE_NO_PAD.decode(s).map_err(Error::other)?;
        Ok(Bytes::copy_from_slice(&v))
    }

    fn to_decode<T>(&self) -> Result<T>
    where
        T: serde::de::DeserializeOwned,
    {
        match rmp_serde::from_slice::<T>(self) {
            Ok(out) => Ok(out),
            Err(err) => Err(Error::other(err)),
        }
    }

    fn to_b64(&self) -> String {
        use base64::prelude::*;
        BASE64_URL_SAFE_NO_PAD.encode(self)
    }
}
