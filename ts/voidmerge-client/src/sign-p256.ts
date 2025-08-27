import * as types from "./types.js";
import { p256 } from "@noble/curves/nist.js";

const ALG: string = "p256";

/**
 * Implements cryptographic signatures using the nist p256 curve.
 */
export class VmSignP256 implements types.VmSign {
  alg(): string {
    return ALG;
  }

  genSecret(): types.VmSignSecretKey {
    const { secretKey } = p256.keygen();
    return types.VmSignSecretKey.fromParts(ALG, secretKey);
  }

  genPublic(secret: types.VmSignSecretKey): types.VmSignPublicKey {
    const pub = p256.getPublicKey(secret.material());
    return types.VmSignPublicKey.fromParts(ALG, pub);
  }

  sign(secret: types.VmSignSecretKey, data: Uint8Array): types.VmSignature {
    const sig = p256.sign(data, secret.material(), { prehash: true });
    return types.VmSignature.fromParts(ALG, sig);
  }

  verify(
    sig: types.VmSignature,
    pub: types.VmSignPublicKey,
    data: Uint8Array,
  ): boolean {
    return p256.verify(sig.material(), data, pub.material(), { prehash: true });
  }
}
