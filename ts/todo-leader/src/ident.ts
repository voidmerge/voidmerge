import { b64Enc, b64Dec } from "./b64.js";
import * as ed from "@noble/ed25519";

const A = "ABCDEFGHJKMNPQRSTUVWXYZabcdefghjkmnpqrstuvwxyz23456789";
const D = "23456789";

const STORE = "TodoLeader";

export class Ident {
  #pk: string;
  #sk: string;
  #short: string;
  #avatarCode: string;

  private constructor(pk: string, sk: string, avatarCode: Uint8Array) {
    if (avatarCode.byteLength !== 16) {
      throw new Error("invalid avatar code");
    }

    const ident = b64Dec(pk);
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

    this.#pk = pk;
    this.#sk = sk;
    this.#short = short;
    this.#avatarCode = b64Enc(avatarCode);
  }

  static async random(): Promise<Ident> {
    const { secretKey, publicKey } = await ed.keygenAsync();
    const pk = b64Enc(publicKey);
    const sk = b64Enc(secretKey);

    const avatarCode = crypto.getRandomValues(new Uint8Array(16));
    return new Ident(pk, sk, avatarCode);
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
      typeof parsed.pk !== "string" ||
      typeof parsed.sk !== "string" ||
      typeof parsed.avatar !== "string"
    ) {
      return;
    }
    return new Ident(parsed.pk, parsed.sk, b64Dec(parsed.avatar));
  }

  async store() {
    localStorage.setItem(
      STORE,
      JSON.stringify({
        pk: this.#pk,
        sk: this.#sk,
        avatar: this.#avatarCode,
      }),
    );
  }

  async sign(input: {
    league: number;
    stars: number;
    weekId: string;
  }): Promise<{
    path: string;
    data: any[];
  }> {
    const fixLeague = (input.league | 0).toString();
    const fixStars = (input.stars | 0).toString();

    const path = `stars~${this.#pk}`;

    const toSign = new TextEncoder().encode(
      JSON.stringify([this.#pk, input.weekId, fixLeague, fixStars]),
    );

    const sig = b64Enc(await ed.signAsync(toSign, b64Dec(this.#sk)));

    const data = [input.weekId, fixLeague, fixStars, sig, this.#avatarCode];

    return { path, data };
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
        avatarCode: this.#avatarCode,
      },
      null,
      2,
    );
    return `Ident(${inner})`;
  }
}
