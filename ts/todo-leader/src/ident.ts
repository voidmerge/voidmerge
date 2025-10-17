import { b64Enc, b64Dec } from "./b64.js";

const A = "ABCDEFGHJKMNPQRSTUVWXYZabcdefghjkmnpqrstuvwxyz23456789";
const D = "23456789";

const STORE = "TodoLeader";

export class Ident {
  #pub: CryptoKey;
  #sec: CryptoKey;
  #ident: string;
  #short: string;
  #avatarCode: string;

  private constructor(
    pub: CryptoKey,
    sec: CryptoKey,
    ident: Uint8Array,
    avatarCode: Uint8Array,
  ) {
    if (ident.byteLength < 16) {
      throw new Error("pub is too short");
    }
    if (avatarCode.byteLength !== 16) {
      throw new Error("invalid avatar code");
    }

    const short =
      A[ident[6] % A.length] +
      A[ident[7] % A.length] +
      D[ident[8] % D.length] +
      "-" +
      A[ident[9] % A.length] +
      A[ident[10] % A.length] +
      D[ident[11] % D.length] +
      "-" +
      A[ident[12] % A.length] +
      A[ident[13] % A.length] +
      D[ident[14] % D.length] +
      A[ident[15] % A.length];

    this.#pub = pub;
    this.#sec = sec;
    this.#ident = b64Enc(ident);
    this.#short = short;
    this.#avatarCode = b64Enc(avatarCode);
  }

  static async random(): Promise<Ident> {
    const pair = await crypto.subtle.generateKey(
      {
        name: "ECDSA",
        namedCurve: "P-256",
      },
      true,
      ["sign", "verify"],
    );
    const ident = new Uint8Array(
      await crypto.subtle.exportKey("raw", pair.publicKey),
    );
    const avatarCode = crypto.getRandomValues(new Uint8Array(16));
    return new Ident(pair.publicKey, pair.privateKey, ident, avatarCode);
  }

  static async load(): Promise<Ident | undefined> {
    const raw = localStorage.getItem(STORE);
    if (!raw) {
      return;
    }
    const parsed = JSON.parse(raw);
    if (
      !parsed ||
      typeof parsed !== "object" ||
      typeof parsed.pub !== "object" ||
      typeof parsed.sec !== "object" ||
      typeof parsed.avatar !== "string"
    ) {
      return;
    }
    const pubK = await crypto.subtle.importKey(
      "jwk",
      parsed.pub,
      {
        name: "ECDSA",
        namedCurve: "P-256",
      },
      true,
      ["verify"],
    );
    const secK = await crypto.subtle.importKey(
      "jwk",
      parsed.sec,
      {
        name: "ECDSA",
        namedCurve: "P-256",
      },
      true,
      ["sign"],
    );
    const ident = new Uint8Array(await crypto.subtle.exportKey("raw", pubK));
    return new Ident(pubK, secK, ident, b64Dec(parsed.avatar));
  }

  async store() {
    const pub = await crypto.subtle.exportKey("jwk", this.#pub);
    const sec = await crypto.subtle.exportKey("jwk", this.#sec);
    localStorage.setItem(
      STORE,
      JSON.stringify({
        pub,
        sec,
        avatar: this.#avatarCode,
      }),
    );
  }

  ident(): string {
    return this.#ident;
  }

  short(): string {
    return this.#short;
  }

  randomizeAvatar() {
    this.#avatarCode = b64Enc(crypto.getRandomValues(new Uint8Array(16)));
    this.store();
  }

  avatarCode(): string {
    return this.#avatarCode;
  }

  debug(): string {
    const inner = JSON.stringify(
      {
        short: this.#short,
        ident: this.#ident,
        avatarCode: this.#avatarCode,
      },
      null,
      2,
    );
    return `Ident(${inner})`;
  }
}
