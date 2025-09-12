use super::*;

/// Signing module implementing the p256 algorithm.
pub struct CryptoSignP256;

impl std::fmt::Debug for CryptoSignP256 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CryptoSignP256").finish()
    }
}

impl CryptoSignP256 {
    /// Construct a new [CryptoSignP256] instance.
    pub fn new() -> Result<Self> {
        Ok(Self)
    }
}

impl CryptoSign for CryptoSignP256 {
    fn alg(&self) -> &'static str {
        "p256"
    }

    fn generate(&self) -> Result<(CryptoSignPublic, CryptoSignSecret)> {
        let secret = ::p256::ecdsa::SigningKey::random(&mut rand::thread_rng());
        let public = secret.verifying_key().to_encoded_point(true);
        let secret: Vec<u8> = secret.to_bytes().into_iter().collect();

        Ok((
            bytes::Bytes::copy_from_slice(public.as_bytes()).into(),
            secret.into(),
        ))
    }

    fn sign_prehashed_512_bits(
        &self,
        sk: &CryptoSignSecret,
        hash: &[u8],
    ) -> Result<CryptoSignature> {
        use ::p256::ecdsa::signature::*;

        if hash.len() != 64 {
            return Err(std::io::Error::other(
                "to sign was not a 512 bit hash",
            ));
        }

        let secret = ::p256::ecdsa::SigningKey::from_slice(&sk).unwrap();

        let sig: ::p256::ecdsa::Signature = secret.sign(hash);

        Ok(bytes::Bytes::from(sig.to_vec()).into())
    }

    fn verify_prehashed_512_bits(
        &self,
        pk: &CryptoSignPublic,
        signature: &CryptoSignature,
        hash: &[u8],
    ) -> Result<()> {
        use ::p256::ecdsa::signature::*;

        if hash.len() != 64 {
            return Err(std::io::Error::other(
                "to verify was not a 512 bit hash",
            ));
        }

        let pk = ::p256::ecdsa::VerifyingKey::from_encoded_point(
            &(&pk[..]).try_into().unwrap(),
        )
        .unwrap();

        let signature =
            ::p256::ecdsa::Signature::from_slice(&signature).unwrap();

        pk.verify(hash, &signature).map_err(std::io::Error::other)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn p256_signature() {
        let alg = CryptoSignP256;
        let (p, s) = alg.generate().unwrap();
        let sig = alg.sign_prehashed_512_bits(&s, &[2; 64]).unwrap();
        alg.verify_prehashed_512_bits(&p, &sig, &[2; 64]).unwrap();
        assert!(alg.verify_prehashed_512_bits(&p, &sig, &[3; 64]).is_err());
    }
}
