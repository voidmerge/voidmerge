use super::*;
use crate::types::*;

/// Signing module implementing the ed25519 algorithm.
pub struct CryptoSignEd25519;

impl std::fmt::Debug for CryptoSignEd25519 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CryptoSignEd25519").finish()
    }
}

impl CryptoSignEd25519 {
    /// Construct a new [CryptoSignEd25519] instance.
    pub fn new() -> Result<Self> {
        Ok(Self)
    }
}

impl CryptoSign for CryptoSignEd25519 {
    fn alg(&self) -> &'static str {
        "ed25519"
    }

    fn generate(&self) -> Result<(CryptoSignPublic, CryptoSignSecret)> {
        let sk = ed25519_dalek::SigningKey::generate(&mut rand::thread_rng());
        let pk = sk.verifying_key();
        Ok((
            bytes::Bytes::copy_from_slice(pk.as_bytes()).into(),
            sk.as_bytes().to_vec().into(),
        ))
    }

    fn sign_prehashed_512_bits(
        &self,
        sk: &CryptoSignSecret,
        hash: &[u8],
    ) -> Result<CryptoSignature> {
        if hash.len() != 64 {
            return Err(std::io::Error::other(
                "to sign was not a 512 bit hash",
            ));
        }
        let sk: [u8; 32] = match (&sk[..]).try_into() {
            Ok(sk) => sk,
            Err(err) => {
                return Err(std::io::Error::other(err)
                    .with_info("invalid signing key".into()));
            }
        };
        let sk = ed25519_dalek::SigningKey::from_bytes(&sk);
        let sig = sk
            .sign_prehashed(Digest(hash), None)
            .map_err(std::io::Error::other)?;
        Ok(bytes::Bytes::copy_from_slice(&sig.to_vec()).into())
    }

    fn verify_prehashed_512_bits(
        &self,
        pk: &CryptoSignPublic,
        signature: &CryptoSignature,
        hash: &[u8],
    ) -> Result<()> {
        if hash.len() != 64 {
            return Err(std::io::Error::other(
                "to verify was not a 512 bit hash",
            ));
        }
        let pk: [u8; 32] = match (&pk[..]).try_into() {
            Ok(pk) => pk,
            Err(err) => {
                return Err(std::io::Error::other(err)
                    .with_info("invalid public key".into()));
            }
        };
        let pk = ed25519_dalek::VerifyingKey::from_bytes(&pk)
            .map_err(std::io::Error::other)?;
        let sig: [u8; 64] = match (&signature[..]).try_into() {
            Ok(sig) => sig,
            Err(err) => {
                return Err(std::io::Error::other(err)
                    .with_info("invalid signature".into()));
            }
        };
        pk.verify_prehashed(
            Digest(hash),
            None,
            &ed25519_dalek::Signature::from_bytes(&sig),
        )
        .map_err(std::io::Error::other)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn ed25519_signature() {
        let alg = CryptoSignEd25519;
        let (p, s) = alg.generate().unwrap();
        let sig = alg.sign_prehashed_512_bits(&s, &[2; 64]).unwrap();
        alg.verify_prehashed_512_bits(&p, &sig, &[2; 64]).unwrap();
        assert!(alg.verify_prehashed_512_bits(&p, &sig, &[3; 64]).is_err());
    }
}
