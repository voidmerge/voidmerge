import * as b64 from "./b64";
import { unpack } from "msgpackr/unpack";
import { pack } from "msgpackr/pack";
import { sha512 as sha2_512 } from "@noble/hashes/sha2";

function deepEqual(a: Uint8Array, b: Uint8Array): boolean {
  if (a.byteLength != b.byteLength) {
    return false;
  }
  for (let i = 0; i < a.byteLength; ++i) {
    if (a[i] !== b[i]) {
      return false;
    }
  }
  return true;
}

/**
 * VoidMerge Hash type.
 */
export class VmHash {
  #data: Uint8Array;

  /**
   * Basic hash constructor.
   */
  constructor(data: Uint8Array) {
    this.#data = data;
  }

  /**
   * Generate an empty (0 length) VmHash.
   */
  static empty(): VmHash {
    return new VmHash(new Uint8Array(0));
  }

  /**
   * A canonical VoidMerge nonce is 24 random bytes.
   */
  static nonce(): VmHash {
    const out = new Uint8Array(24);
    return new VmHash(crypto.getRandomValues(out));
  }

  /**
   * Parse a stringified VmHash.
   */
  static parse(s: string): VmHash {
    return new VmHash(b64.decode(s));
  }

  /**
   */
  isEmpty(): boolean {
    return this.#data.byteLength === 0;
  }

  /**
   */
  truncated(len: number): VmHash {
    if (this.#data.byteLength <= len) {
      return this;
    }
    return new VmHash(this.#data.subarray(0, len));
  }

  /**
   * Get the raw Uint8Array data.
   */
  data(): Uint8Array {
    return this.#data;
  }

  /**
   * Stringify this hash into the canonical base64url repr.
   */
  toString(): string {
    return b64.encode(this.#data);
  }

  /**
   * Also use the string repr for JSON.
   */
  toJSON(): string {
    return this.toString();
  }

  /**
   * Try to show the canonical repr when debugging.
   */
  get [Symbol.toStringTag]() {
    return this.toString();
  }
}

const B_PUB: Uint8Array = new Uint8Array([166, 230, 254]);
const B_SEC: Uint8Array = new Uint8Array([177, 231, 62]);
const B_SIG: Uint8Array = new Uint8Array([178, 40, 62]);

/**
 * A base encoding/decoding type for cryptographic signing data.
 */
export class VmSignType {
  #data: Uint8Array;

  constructor(data: Uint8Array) {
    this.#data = data;
  }

  /**
   */
  static fromParts(
    alg: string,
    typ: Uint8Array,
    material: Uint8Array,
  ): VmSignType {
    const algBin = b64.decode(alg);
    if (algBin.byteLength !== 3) {
      throw new TypeError("alg must be exactly 3 bytes (4 b64 chars)");
    }
    if (typ.byteLength !== 3) {
      throw new TypeError("typ must be exactly 3 bytes (4 b64 chars)");
    }
    const data = new Uint8Array(3 + 3 + material.byteLength);
    data.set(algBin);
    data.set(typ, 3);
    data.set(material, 6);
    return new VmSignType(data);
  }

  /**
   */
  static parse(encoded: string): VmSignType {
    return new VmSignType(b64.decode(encoded));
  }

  /**
   * Get the full encoded version of this type.
   */
  encode(): Uint8Array {
    return this.#data;
  }

  /**
   * Stringify this type into the base64url repr.
   */
  toString(): string {
    return b64.encode(this.#data);
  }

  /**
   * Also use the string repr for JSON.
   */
  toJSON(): string {
    return this.toString();
  }

  /**
   * Try to show the string repr when debugging.
   */
  get [Symbol.toStringTag]() {
    if (deepEqual(this.typ(), B_SEC)) {
      return b64.encode(this.#data.subarray(0, 6)) + "<secret>";
    } else {
      return this.toString();
    }
  }

  /**
   * Get the algorithm.
   */
  alg(): string {
    return b64.encode(this.#data.subarray(0, 3));
  }

  /**
   * Get the typ.
   */
  typ(): Uint8Array {
    return this.#data.subarray(3, 6);
  }

  /**
   * Get the material portion of this type.
   */
  material(): Uint8Array {
    return this.#data.subarray(6);
  }

  /**
   * Is this a public key?
   */
  isPublic(): boolean {
    if (deepEqual(this.typ(), B_PUB)) {
      return true;
    } else {
      return false;
    }
  }

  /**
   * Convert this to a public key.
   */
  toPublic(): VmSignPublicKey {
    if (!this.isPublic()) {
      throw new TypeError("VmSignType is not a public key");
    }
    return new VmSignPublicKey(this.#data);
  }

  /**
   * Is this a secret key?
   */
  isSecret(): boolean {
    if (deepEqual(this.typ(), B_SEC)) {
      return true;
    } else {
      return false;
    }
  }

  /**
   * Convert this to a public key.
   */
  toSecret(): VmSignSecretKey {
    if (!this.isSecret()) {
      throw new TypeError("VmSignType is not a secret key");
    }
    return new VmSignSecretKey(this.#data);
  }

  /**
   * Is this a signature?
   */
  isSignature(): boolean {
    if (deepEqual(this.typ(), B_SIG)) {
      return true;
    } else {
      return false;
    }
  }

  /**
   * Convert this to a public key.
   */
  toSignature(): VmSignature {
    if (!this.isSignature()) {
      throw new TypeError("VmSignType is not a signature");
    }
    return new VmSignature(this.#data);
  }
}

/**
 * A public signature key.
 */
export class VmSignPublicKey extends VmSignType {
  constructor(data: Uint8Array) {
    super(data);
  }

  /**
   */
  static parse(encoded: string): VmSignType {
    return VmSignType.parse(encoded).toPublic();
  }

  /**
   * Construct a public key from components.
   */
  static fromParts(alg: string, material: Uint8Array): VmSignPublicKey {
    return VmSignType.fromParts(alg, B_PUB, material).toPublic();
  }
}

/**
 * A secret signature key
 */
export class VmSignSecretKey extends VmSignType {
  constructor(data: Uint8Array) {
    super(data);
  }

  /**
   */
  static parse(encoded: string): VmSignType {
    return VmSignType.parse(encoded).toSecret();
  }

  /**
   * Construct a secret key from components.
   */
  static fromParts(alg: string, material: Uint8Array): VmSignSecretKey {
    return VmSignType.fromParts(alg, B_SEC, material).toSecret();
  }
}

/**
 * A cryptographic signature.
 */
export class VmSignature extends VmSignType {
  constructor(data: Uint8Array) {
    super(data);
  }

  /**
   */
  static parse(encoded: string): VmSignType {
    return VmSignType.parse(encoded).toSignature();
  }

  /**
   * Construct a signature from components.
   */
  static fromParts(alg: string, material: Uint8Array): VmSignature {
    return VmSignType.fromParts(alg, B_SIG, material).toSignature();
  }
}

/**
 * Represenst an algorithm for doing cryptographic signing.
 */
export interface VmSign {
  /**
   * The name of this signing algorithm.
   */
  alg(): string;

  /**
   * Generate a new random secret key.
   */
  genSecret(): VmSignSecretKey;

  /**
   * Generate a public key from a secret key.
   */
  genPublic(secret: VmSignSecretKey): VmSignPublicKey;

  /**
   * Sign some data with a secret key.
   */
  sign(secret: VmSignSecretKey, data: Uint8Array): VmSignature;

  /**
   * Verify a signature against the data that was signed.
   */
  verify(sig: VmSignature, pub: VmSignPublicKey, data: Uint8Array): boolean;
}

interface VmMultiSignAlg {
  sign: VmSign;
  sk: VmSignSecretKey;
  pk: VmSignPublicKey;
}

/**
 * Generate and verify multiple signatures.
 */
export class VmMultiSign {
  #mods: Array<VmSign>;
  #alg: null | { [key: string]: VmMultiSignAlg };

  constructor() {
    this.#mods = [];
    this.#alg = null;
  }

  private getAlgMap(): { [key: string]: VmMultiSignAlg } {
    if (!this.#alg) {
      this.#alg = {};
      for (const sign of this.#mods) {
        const sk = sign.genSecret();
        const pk = sign.genPublic(sk);
        this.#alg[sign.alg()] = { sign, sk, pk };
      }
    }
    return this.#alg;
  }

  /**
   * Load encoded key material data from persistence.
   */
  loadEncoded(encoded: string) {
    const parsed: Array<string> = JSON.parse(encoded);
    const parsed2: { [key: string]: VmSignSecretKey } = {};
    for (const s of parsed) {
      const sk = VmSignSecretKey.parse(s);
      parsed2[sk.alg()] = sk;
    }
    this.#alg = {};
    for (const sign of this.#mods) {
      const alg = sign.alg();
      if (!(alg in parsed2)) {
        throw new Error(`no secret key found for alg ${alg}`);
      }
      const sk = parsed2[alg];
      const pk = sign.genPublic(sk);
      this.#alg[alg] = { sign, sk, pk };
    }
  }

  /**
   * Encode key material data for persistence.
   */
  encode(): string {
    const algMap = this.getAlgMap();
    const out: Array<string> = [];
    for (const alg in algMap) {
      out.push(algMap[alg].sk.toString());
    }
    return JSON.stringify(out);
  }

  /**
   * Add a signing module to this multi sign instance.
   */
  addSign(sign: VmSign) {
    this.#mods.push(sign);
  }

  /**
   * Sign some data
   */
  sign(data: Uint8Array): Array<VmSignaturePkg> {
    const algMap = this.getAlgMap();
    const out = [];
    for (const alg in algMap) {
      const algRef = algMap[alg];
      const sig = algRef.sign.sign(algRef.sk, data);
      out.push({ pk: algRef.pk, sig });
    }
    return out;
  }
}

/**
 */
export interface VmAuthChalReq {
  token: VmHash;
  nonce: VmHash;
}

/**
 */
export interface VmAuthChalRes {
  nonceSig: Array<VmSignaturePkg>;
  contextAccess: Array<[VmHash, any]>;
}

/**
 */
export interface VmMsg {
  ctx: VmHash;
  peer: VmHash;
  data: Uint8Array;
}

/**
 * Cache of VmObjSigneds indexed by short hash.
 */
export interface VmObjSignedShortCache {
  put(bundle: VmObjSigned): void;
  getByShort(short: VmHash): null | VmObjSigned;
}

/**
 */
export class VmObjSignedShortCacheLru implements VmObjSignedShortCache {
  #maxBytes: number;
  #storedBytes: number;
  #byShort: { [key: string]: { bundle: VmObjSigned; time: number } };

  /**
   * Construct an LRU cache that keeps the specified byte count in memory.
   */
  constructor(maxBytes: number) {
    this.#maxBytes = maxBytes;
    this.#storedBytes = 0;
    this.#byShort = {};
  }

  private prune(): void {
    if (this.#storedBytes <= this.#maxBytes) {
      return;
    }
    const shortList = Object.keys(this.#byShort).sort((a, b) => {
      return this.#byShort[a].time - this.#byShort[b].time;
    });
    while (this.#storedBytes > this.#maxBytes) {
      const next = shortList.shift();
      if (!next) {
        break;
      }
      const item = this.#byShort[next];
      delete this.#byShort[next];
      this.#storedBytes -= item.bundle.enc.byteLength;
    }
  }

  put(bundle: VmObjSigned): void {
    let short = bundle.sha512.truncated(24).toString();
    if (short in this.#byShort) {
      return;
    }
    this.#storedBytes += bundle.enc.byteLength;
    this.#byShort[short] = { bundle, time: Date.now() };
    this.prune();
  }

  getByShort(short: VmHash): null | VmObjSigned {
    const out = this.#byShort[short.toString()];
    if (out) {
      out.time = Date.now();
      return out.bundle;
    }
    return null;
  }
}

export class VmObj {
  type: string;
  ident?: VmHash;
  deps?: Array<VmHash>;
  ttlS?: number;
  app?: any;

  constructor(type: string) {
    this.type = type;
  }

  private static decodeObject(dec: {
    type?: string;
    ident?: VmHash;
    deps?: Array<VmHash>;
    ttlS?: number;
    app?: any;
  }): VmObj {
    if (typeof dec.type !== "string") {
      throw new TypeError("dec.type must be a string");
    }

    const out = new VmObj(dec.type);

    if (dec.ident instanceof Uint8Array) {
      out.ident = new VmHash(dec.ident);
    }

    if (Array.isArray(dec.deps)) {
      out.deps = [];
      for (const dep of dec.deps) {
        if (dep instanceof Uint8Array) {
          out.deps.push(new VmHash(dep));
        }
      }
    }

    if (typeof dec.ttlS === "number") {
      out.ttlS = dec.ttlS;
    }

    if (dec.app) {
      out.app = dec.app;
    }

    return out;
  }

  private static decodeBinary(dec: Uint8Array): VmObj {
    const parsed = unpack(dec);
    if (!parsed || typeof parsed !== "object") {
      throw new TypeError("invalid decode param");
    }
    return VmObj.decodeObject(parsed);
  }

  static decode(dec: any): VmObj {
    if (!dec) {
      throw new TypeError("invalid decode param");
    }

    if (dec instanceof Uint8Array) {
      return VmObj.decodeBinary(dec);
    }

    if (dec.buffer instanceof ArrayBuffer) {
      return VmObj.decodeBinary(new Uint8Array(dec.buffer));
    }

    if (typeof dec === "object") {
      return VmObj.decodeObject(dec);
    }

    throw new TypeError("invalid decode param");
  }

  encode(): Uint8Array {
    const out: {
      type: string;
      ident?: Uint8Array;
      deps?: Array<Uint8Array>;
      ttlS?: number;
      app?: any;
    } = { type: this.type };

    if (this.ident) {
      out.ident = this.ident.data();
    }

    if (this.deps) {
      out.deps = [];
      for (const dep of this.deps) {
        out.deps.push(dep.data());
      }
    }

    if (this.ttlS) {
      out.ttlS = this.ttlS;
    }

    if (this.app) {
      out.app = this.app;
    }

    return pack(out);
  }

  withIdent(ident: VmHash): VmObj {
    this.ident = ident;
    return this;
  }

  withDeps(deps: Array<VmHash>): VmObj {
    this.deps = deps;
    return this;
  }

  withTtlS(ttlS: number): VmObj {
    this.ttlS = ttlS;
    return this;
  }

  withApp(app: any): VmObj {
    this.app = app;
    return this;
  }

  sign(sign: VmMultiSign): VmObjSigned {
    const enc = this.encode();

    const sha512 = sha2_512(enc);

    const sigs = sign.sign(sha512);

    return new VmObjSigned(this, enc, new VmHash(sha512), sigs);
  }
}

export interface VmSignaturePkg {
  pk: VmSignPublicKey;
  sig: VmSignature;
}

export class VmObjSigned {
  parsed: VmObj;
  enc: Uint8Array;
  sha512: VmHash;
  sigs: Array<VmSignaturePkg>;

  constructor(
    parsed: VmObj,
    enc: Uint8Array,
    sha512: VmHash,
    sigs: Array<VmSignaturePkg>,
  ) {
    this.parsed = parsed;
    this.enc = enc;
    this.sha512 = sha512;
    this.sigs = sigs;
  }

  encode(): Uint8Array {
    const sigs: Array<{ pk: Uint8Array; sig: Uint8Array }> = [];
    for (const sig of this.sigs) {
      sigs.push({
        pk: sig.pk.encode(),
        sig: sig.sig.encode(),
      });
    }

    const out: {
      enc: Uint8Array;
      sha512: Uint8Array;
      sigs: Array<{ pk: Uint8Array; sig: Uint8Array }>;
    } = {
      enc: this.enc,
      sha512: this.sha512.data(),
      sigs,
    };

    return pack(out);
  }

  private static decodeObject(dec: {
    enc?: Uint8Array;
    sha512?: Uint8Array;
    sigs?: Array<{ pk?: Uint8Array; sig?: Uint8Array }>;
  }): VmObjSigned {
    if (!(dec.enc instanceof Uint8Array)) {
      throw new TypeError("dec.enc must be a Uint8Array");
    }
    if (!(dec.sha512 instanceof Uint8Array)) {
      throw new TypeError("dec.sha512 must be a Uint8Array");
    }
    if (!Array.isArray(dec.sigs)) {
      throw new TypeError("dec.sigs must be an array");
    }

    const sigs = [];
    for (const sig of dec.sigs) {
      if (!(sig.pk instanceof Uint8Array)) {
        throw new TypeError("dec.sigs[].pk must be a Uint8Array");
      }
      if (!(sig.sig instanceof Uint8Array)) {
        throw new TypeError("dec.sigs[].sig must be a Uint8Array");
      }
      sigs.push({
        pk: new VmSignType(sig.pk).toPublic(),
        sig: new VmSignType(sig.sig).toSignature(),
      });
    }

    let parsed = VmObj.decode(dec.enc);

    return new VmObjSigned(parsed, dec.enc, new VmHash(dec.sha512), sigs);
  }

  private static decodeBinary(dec: Uint8Array): VmObjSigned {
    const parsed = unpack(dec);
    if (!parsed || typeof parsed !== "object") {
      throw new TypeError("invalid decode param");
    }
    return VmObjSigned.decodeObject(parsed);
  }

  static decode(dec: any): VmObjSigned {
    if (!dec) {
      throw new TypeError("invalid decode param");
    }

    if (dec instanceof Uint8Array) {
      return VmObjSigned.decodeBinary(dec);
    }

    if (dec.buffer instanceof ArrayBuffer) {
      return VmObjSigned.decodeBinary(new Uint8Array(dec.buffer));
    }

    if (typeof dec === "object") {
      return VmObjSigned.decodeObject(dec);
    }

    throw new TypeError("invalid decode param");
  }
}

/**
 * VoidMerge logic as a single utf8 string.
 */
export interface VmLogicUtf8Single {
  type: "utf8Single";
  code: string;
}

/**
 */
export class VmSelectResponse {
  count: number;
  results?: Array<{
    type?: string;
    short?: VmHash;
    ident?: VmHash;
    data?: VmObjSigned;
  }>;

  constructor(count: number) {
    this.count = count;
  }

  private static decodeObject(dec: {
    count?: number;
    results?: Array<{
      type?: string;
      short?: VmHash;
      ident?: VmHash;
      data?: VmObjSigned;
    }>;
  }): VmSelectResponse {
    if (typeof dec.count !== "number") {
      throw new TypeError("dec.count must be a number");
    }

    const out = new VmSelectResponse(dec.count);

    if (Array.isArray(dec.results)) {
      out.results = [];

      for (const item of dec.results) {
        const outItem: {
          type?: string;
          short?: VmHash;
          ident?: VmHash;
          data?: VmObjSigned;
        } = {};

        if (typeof item.type === "string") {
          outItem.type = item.type;
        }

        if (item.short instanceof Uint8Array) {
          outItem.short = new VmHash(item.short);
        }

        if (item.ident instanceof Uint8Array) {
          outItem.ident = new VmHash(item.ident);
        }

        if (item.data) {
          outItem.data = VmObjSigned.decode(item.data);
        }

        out.results.push(outItem);
      }
    }

    return out;
  }

  private static decodeBinary(dec: Uint8Array): VmSelectResponse {
    const parsed = unpack(dec);
    if (!parsed || typeof parsed !== "object") {
      throw new TypeError("invalid decode param");
    }
    return VmSelectResponse.decodeObject(parsed);
  }

  static decode(dec: any): VmSelectResponse {
    if (!dec) {
      throw new TypeError("invalid decode param");
    }

    if (dec instanceof Uint8Array) {
      return VmSelectResponse.decodeBinary(dec);
    }

    if (dec.buffer instanceof ArrayBuffer) {
      return VmSelectResponse.decodeBinary(new Uint8Array(dec.buffer));
    }

    if (typeof dec === "object") {
      return VmSelectResponse.decodeObject(dec);
    }

    throw new TypeError("invalid decode param");
  }
}

/**
 * Construct data for selecting (querying) data from a VoidMerge instance.
 */
export class VmSelect {
  filterByTypes?: Array<string>;
  filterByIdents?: Array<VmHash>;
  filterByShorts?: Array<VmHash>;
  returnShort?: boolean;
  returnIdent?: boolean;
  returnType?: boolean;
  returnData?: boolean;

  /**
   */
  encode(): Uint8Array {
    const enc: {
      filterByTypes?: Array<string>;
      filterByIdents?: Array<Uint8Array>;
      filterByShorts?: Array<Uint8Array>;
      returnShort?: boolean;
      returnIdent?: boolean;
      returnType?: boolean;
      returnData?: boolean;
    } = {};

    if (Array.isArray(this.filterByTypes)) {
      enc.filterByTypes = this.filterByTypes;
    }

    if (Array.isArray(this.filterByIdents)) {
      enc.filterByIdents = [];
      for (const ident of this.filterByIdents) {
        enc.filterByIdents.push(ident.data());
      }
    }

    if (Array.isArray(this.filterByShorts)) {
      enc.filterByShorts = [];
      for (const short of this.filterByShorts) {
        enc.filterByShorts.push(short.data());
      }
    }

    if (this.returnShort === true) {
      enc.returnShort = true;
    }

    if (this.returnIdent === true) {
      enc.returnIdent = true;
    }

    if (this.returnType === true) {
      enc.returnType = true;
    }

    if (this.returnData === true) {
      enc.returnData = true;
    }

    return pack(enc);
  }

  /**
   * By default, select will return items of all types.
   * If you would like to limit this, specify a list of types to include.
   */
  withFilterByTypes(types: Array<string>): VmSelect {
    this.filterByTypes = types;
    return this;
  }

  /**
   * If you would like to only return items with specific idents,
   * specify that list of idents here.
   */
  withFilterByIdents(idents: Array<VmHash>): VmSelect {
    this.filterByIdents = idents;
    return this;
  }

  /**
   */
  withFilterByShorts(shorts: Array<VmHash>): VmSelect {
    this.filterByShorts = shorts;
    return this;
  }

  /**
   * By default, select results do not include the short hash.
   * If you set returnShort to true, these hashes will be included.
   */
  withReturnShort(returnShort: boolean): VmSelect {
    this.returnShort = returnShort;
    return this;
  }

  /**
   * By default, select results do not include the ident.
   * If you set returnIdent to true, these idents will be included.
   */
  withReturnIdent(returnIdent: boolean): VmSelect {
    this.returnIdent = returnIdent;
    return this;
  }

  /**
   * By default, select results do not include the type of the item.
   * If you set returnType to true, the type will be included with the items.
   */
  withReturnType(returnType: boolean): VmSelect {
    this.returnType = returnType;
    return this;
  }

  /**
   * By default, select results do not include the actualy data content.
   * If you set returnData to true, this will be included in the result.
   * Note that this may result in a very large response that may get
   * truncated. Instead, you could fetch a list of short hashes, and
   * then make separate individual requests for the content data.
   */
  withReturnData(returnData: boolean): VmSelect {
    this.returnData = returnData;
    return this;
  }
}
