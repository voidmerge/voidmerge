use super::*;
use crate::types::*;

macro_rules! mk_sign {
    ($a:literal, $i:ident, $t:path) => {
        #[doc = concat!(
            "Signing module implementing the ",
            $a,
            r#" algorithm.

Note: This ml-dsa module does not respect the prehashed trait hints,
      and instead uses the prehashed bytes as normal siging data input."#,
        )]
        pub struct $i(oqs::sig::Sig);

        impl std::fmt::Debug for $i {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.debug_struct(stringify!($i)).finish()
            }
        }

        impl $i {
            #[doc = concat!("Construct a new [", stringify!($i), "] instance.")]
            pub fn new() -> Result<Self> {
                Ok(Self(oqs::sig::Sig::new($t).map_err(|err| {
                    std::io::Error::other(err).with_info(
                        concat!(
                            "error constructing ",
                            stringify!($i),
                            " algorithm instance",
                        )
                        .into(),
                    )
                })?))
            }
        }

        impl CryptoSign for $i {
            fn alg(&self) -> &'static str {
                $a
            }

            fn generate(&self) -> Result<(CryptoSignPublic, CryptoSignSecret)> {
                let (pk, sk) = self.0.keypair().map_err(|err| {
                    std::io::Error::other(err).with_info(
                        concat!(
                            "error generating ",
                            stringify!($i),
                            " keypair",
                        )
                        .into(),
                    )
                })?;

                Ok((
                    bytes::Bytes::copy_from_slice(pk.as_ref()).into(),
                    sk.into_vec().into(),
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
                let sk = match self.0.secret_key_from_bytes(&sk) {
                    Some(sk) => sk,
                    None => {
                        return Err(std::io::Error::other(
                            "invalid secret key",
                        ));
                    }
                };
                let sig = self.0.sign(hash, sk).map_err(|err| {
                    std::io::Error::other(err).with_info(
                        concat!(
                            "error generating ",
                            stringify!($i),
                            " signature",
                        )
                        .into(),
                    )
                })?;
                Ok(bytes::Bytes::from(sig.into_vec()).into())
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
                let pk = match self.0.public_key_from_bytes(&pk) {
                    Some(pk) => pk,
                    None => {
                        return Err(std::io::Error::other(
                            "invalid public key",
                        ));
                    }
                };
                let sig = match self.0.signature_from_bytes(&signature) {
                    Some(sig) => sig,
                    None => {
                        return Err(std::io::Error::other("invalid signature"));
                    }
                };
                self.0.verify(hash, sig, pk).map_err(|err| {
                    std::io::Error::other(err).with_info(
                        concat!(
                            "error verifying ",
                            stringify!($i),
                            " signature",
                        )
                        .into(),
                    )
                })
            }
        }
    };
}

mk_sign!("ml-dsa-44", CryptoSignMlDsa44, oqs::sig::Algorithm::MlDsa44);

mk_sign!("ml-dsa-65", CryptoSignMlDsa65, oqs::sig::Algorithm::MlDsa65);

mk_sign!("ml-dsa-87", CryptoSignMlDsa87, oqs::sig::Algorithm::MlDsa87);

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn ml_signatures() {
        let algs: Vec<DynCryptoSign> = vec![
            Arc::new(CryptoSignMlDsa44::new().unwrap()),
            Arc::new(CryptoSignMlDsa65::new().unwrap()),
            Arc::new(CryptoSignMlDsa87::new().unwrap()),
        ];
        for alg in algs {
            let (p, s) = alg.generate().unwrap();
            let sig = alg.sign_prehashed_512_bits(&s, &[2; 64]).unwrap();
            alg.verify_prehashed_512_bits(&p, &sig, &[2; 64]).unwrap();
            assert!(alg.verify_prehashed_512_bits(&p, &sig, &[3; 64]).is_err());
        }
    }
}
