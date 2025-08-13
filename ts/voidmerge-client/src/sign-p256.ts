import * as types from "./types";
import { p256 } from "@noble/curves/nist";

const ALG: string = "p256";

/**
 * Implements cryptographic signatures using the nist p256 curve.
 */
export class VmSignP256 implements types.VmSign {
  alg(): string {
    return ALG;
  }

  genSecret(): types.VmSignSecretKey {
    const priv = p256.utils.randomPrivateKey();
    return types.VmSignSecretKey.fromParts(ALG, priv);
  }

  genPublic(secret: types.VmSignSecretKey): types.VmSignPublicKey {
    const pub = p256.getPublicKey(secret.material());
    return types.VmSignPublicKey.fromParts(ALG, pub);
  }

  sign(secret: types.VmSignSecretKey, data: Uint8Array): types.VmSignature {
    const sig = p256.sign(data, secret.material(), { prehash: true });
    return types.VmSignature.fromParts(ALG, sig.toCompactRawBytes());
  }

  verify(
    sig: types.VmSignature,
    pub: types.VmSignPublicKey,
    data: Uint8Array,
  ): boolean {
    const vsig = p256.Signature.fromCompact(sig.material());
    return p256.verify(vsig, data, pub.material(), { prehash: true });
  }
}
