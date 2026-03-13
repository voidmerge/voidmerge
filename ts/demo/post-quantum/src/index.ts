import * as VM from "@voidmerge/voidmerge-code";
import { x25519 } from "@noble/curves/ed25519.js";
import { ml_kem768 } from "@noble/post-quantum/ml-kem.js";
import { sha256 } from "@noble/hashes/sha2.js";
import { fromByteArray } from "base64-js";

/*
 * This is an example of hybrid x25519 + ML-KEM post-quantum key exchange.
 *
 * BE AWARE: This is for example / educational purposes ONLY.
 *
 * Cryptography is highly vulnerable to subtle bugs and vulnerabilities
 * depending specific implementation. Please consult with a professional.
 */

function concat(a: Uint8Array, b: Uint8Array): Uint8Array<ArrayBuffer> {
  const out = new Uint8Array(a.byteLength + b.byteLength);
  out.set(a, 0);
  out.set(b, a.byteLength);
  return out;
}

class Server {
  #xPub: Uint8Array;
  #xSec: Uint8Array;
  #mPub: Uint8Array;
  #mSec: Uint8Array;

  #shared: string;

  constructor() {
    // x25519 keypair
    let { publicKey, secretKey } = x25519.keygen();
    this.#xPub = publicKey;
    this.#xSec = secretKey;

    // ml kem keypair
    ({ publicKey, secretKey } = ml_kem768.keygen());
    this.#mPub = publicKey;
    this.#mSec = secretKey;

    // we calculate shared secret in loadRemotePubKey
    this.#shared = '<unset>';
  }

  pubKey(): Uint8Array<ArrayBuffer> {
    // server pubkey is x25519 pk plus ml kem pk
    return concat(this.#xPub, this.#mPub);
  }

  loadRemotePubKey(pubKey: Uint8Array) {
    const remX = pubKey.subarray(0, 32);
    const remM = pubKey.subarray(32);

    // calculate the shared secrets
    const xShare = x25519.getSharedSecret(this.#xSec, remX);
    const mShare = ml_kem768.decapsulate(remM, this.#mSec);

    // store the joined shared secret allowing encryption / decryption.
    this.#shared = fromByteArray(sha256(concat(xShare, mShare)));
  }

  getShared(): string {
    return this.#shared;
  }
}

class Client {
  #xPub: Uint8Array;
  #xSec: Uint8Array;

  #mCipher: Uint8Array;

  #shared: string;

  constructor(pubKey: Uint8Array) {
    // x25519 keypair
    let { publicKey, secretKey } = x25519.keygen();
    this.#xPub = publicKey;
    this.#xSec = secretKey;

    const remX = pubKey.subarray(0, 32);
    const remM = pubKey.subarray(32);

    // calculate the shared secret
    const xShare = x25519.getSharedSecret(this.#xSec, remX);
    const { cipherText, sharedSecret: mShare } = ml_kem768.encapsulate(remM);

    this.#mCipher = cipherText;

    this.#shared = fromByteArray(sha256(concat(xShare, mShare)));
  }

  pubKey(): Uint8Array<ArrayBuffer> {
    // client pubkey is x25519 pk plus ml kem encapsulated cipher
    return concat(this.#xPub, this.#mCipher);
  }

  getShared(): string {
    return this.#shared;
  }
}

// Main api handler.
VM.onFn(async (req) => {
  try {
    const out = [];

    // create a server
    const server = new Server();

    // create a client, give the client the server's pubKey
    const client = new Client(server.pubKey());

    // client is able to calculate a shared secret
    const clientShared = client.getShared();
    out.push(`client shared secret: ${clientShared}`);

    // send the client's pub key to the server
    server.loadRemotePubKey(client.pubKey());

    // server is able to calculate a shared secret
    const serverShared = server.getShared();
    out.push(`server shared secret: ${serverShared}`);

    // if the algorithms worked, the shared secrets should match
    if (clientShared === serverShared) {
      out.push(`Secret Key Exchange: SUCCESS!`);
    } else {
      out.push(`Secret Key Exchange: FAILED!`);
    }

    // return the response
    return new VM.ResponseFnOk().text(out.join('\r\n'));
  } catch (e: any) {
    try {
      return new VM.ResponseFnOk()
        .withStatus(500)
        .text(`Error: ${e.toString()}`);
    } catch (_: any) {
      throw e;
    }
  }
});
