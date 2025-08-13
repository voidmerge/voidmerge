use crate::*;
use types::*;

const ALG: &[u8] = &[167, 110, 122];

fn gen_secret() -> SignSecretKey {
    let secret = p256::ecdsa::SigningKey::random(&mut rand::thread_rng());
    let secret: Vec<u8> = secret.to_bytes().into_iter().collect();

    SignSecretKey::from_parts(ALG, &secret)
}

fn gen_public(secret: &SignSecretKey) -> SignPublicKey {
    let secret =
        p256::ecdsa::SigningKey::from_slice(secret.material()).unwrap();
    let public = secret.verifying_key().to_encoded_point(true);

    SignPublicKey::from_parts(ALG, public.as_bytes())
}

fn sign(secret: &SignSecretKey, data: &[u8]) -> Signature {
    use p256::ecdsa::signature::*;

    let secret =
        p256::ecdsa::SigningKey::from_slice(secret.material()).unwrap();
    let signature: p256::ecdsa::Signature = secret.sign(data);

    Signature::from_parts(ALG, &signature.to_vec())
}

fn verify(signature: &Signature, pk: &SignPublicKey, data: &[u8]) -> bool {
    use p256::ecdsa::signature::*;
    let pk = p256::ecdsa::VerifyingKey::from_encoded_point(
        &(pk.material()).try_into().unwrap(),
    )
    .unwrap();
    let signature =
        p256::ecdsa::Signature::from_slice(signature.material()).unwrap();
    pk.verify(data, &signature).is_ok()
}

/// P256 signing functions.
#[derive(Debug)]
pub struct SignP256;

impl ModuleSign for SignP256 {
    fn alg(&self) -> &'static str {
        "p256"
    }

    fn gen_secret(&self) -> SignSecretKey {
        gen_secret()
    }

    fn gen_public(&self, secret: &SignSecretKey) -> SignPublicKey {
        gen_public(secret)
    }

    fn sign(&self, secret: &SignSecretKey, data: &[u8]) -> Signature {
        sign(secret, data)
    }

    fn verify(&self, sig: &Signature, pk: &SignPublicKey, data: &[u8]) -> bool {
        verify(sig, pk, data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn p256() {
        let sk = gen_secret();
        let pk = gen_public(&sk);
        let sig = sign(&sk, b"hello");
        assert!(verify(&sig, &pk, b"hello"));
        assert!(!verify(&sig, &pk, b"world"));
    }

    #[test]
    fn p256_fixture() {
        const PK: &str = "p256pub-As9a6YV1Nu884vcTXWa4aipyjdrDp1P8OFRssABzaFIu";
        const SIG: &str = "p256sig-Bbfc4x2laTzDTYl2wuZ103MmjR3wmZpscEMM9mbzNCwbRghbACVP5DNGMywWGulXm99McMDpTbReqRq4ofjlvA";
        const MSG: &[u8] = &[42, 39, 200];
        let pk: SignPublicKey = PK.parse().unwrap();
        let sig: Signature = SIG.parse().unwrap();
        assert!(verify(&sig, &pk, MSG));
    }
}
