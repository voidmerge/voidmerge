import * as types from "./types.js";
import fetch from "cross-fetch";
import { unpack } from "msgpackr/unpack";
import { pack } from "msgpackr/pack";

function toBuf(b: any): Uint8Array {
  if (b instanceof Uint8Array) {
    return b;
  }
  if (b instanceof ArrayBuffer) {
    return new Uint8Array(b);
  }
  throw new TypeError("failed to conver to Uint8Array");
}

function parseMsg(data: Uint8Array): types.VmMsg {
  const parsed: null | {
    ctx?: Uint8Array;
    peer?: Uint8Array;
    data?: Uint8Array;
  } = unpack(data);

  if (!parsed || typeof parsed !== "object") {
    throw new Error("unexpected message response");
  }

  if (!(parsed.ctx instanceof Uint8Array)) {
    throw new Error("msg.ctx must be a Uint8Array");
  }

  if (!(parsed.peer instanceof Uint8Array)) {
    throw new Error("msg.peer must be a Uint8Array");
  }

  if (!(parsed.data instanceof Uint8Array)) {
    throw new Error("msg.data must be a Uint8Array");
  }

  return {
    ctx: new types.VmHash(parsed.ctx),
    peer: new types.VmHash(parsed.peer),
    data: parsed.data,
  };
}

/**
 * Websocket wrapper.
 */
export class VmWebSocket {
  #ws: WebSocket;
  #hash: types.VmHash;
  #buf: Array<types.VmMsg>;
  #msgCb: null | ((msg: types.VmMsg) => void);

  private constructor(
    ws: WebSocket,
    hash: types.VmHash,
    buf: Array<types.VmMsg>,
  ) {
    this.#ws = ws;
    this.#hash = hash;
    this.#buf = buf;
    this.#msgCb = null;
    const self = this;
    this.#ws.onmessage = (evt: any) => {
      if (!evt || typeof evt !== "object") {
        return;
      }
      if (!evt.data) {
        return;
      }
      const data = parseMsg(toBuf(evt.data));
      if (self.#msgCb) {
        self.#msgCb(data);
      } else {
        self.#buf.push(data);
      }
    };
  }

  /**
   * Establish a new WebSocket connection.
   */
  static async connect(url: URL): Promise<VmWebSocket> {
    const ws = new WebSocket(url);
    ws.binaryType = "arraybuffer";
    return await new Promise((res, rej) => {
      const timer = setTimeout(() => rej("failed to open WebSocket"), 5000);
      ws.onerror = (err: any) => {
        clearTimeout(timer);
        err = err.message || err.type || err.toString();
        rej(`ws connect error: ${err}`);
      };
      const result: {
        ws: WebSocket;
        open: boolean;
        hash: types.VmHash;
        buf: Array<types.VmMsg>;
      } = {
        ws,
        open: false,
        hash: types.VmHash.empty(),
        buf: [],
      };
      const checkDone = () => {
        if (result.open && !result.hash.isEmpty()) {
          clearTimeout(timer);
          res(new VmWebSocket(result.ws, result.hash, result.buf));
        }
      };
      ws.onopen = () => {
        result.open = true;
        checkDone();
      };
      ws.onmessage = (evt: any) => {
        if (!evt || typeof evt !== "object") {
          return;
        }
        if (!evt.data) {
          return;
        }
        const data = toBuf(evt.data);
        if (result.hash.isEmpty()) {
          result.hash = new types.VmHash(data);
        } else {
          result.buf.push(parseMsg(data));
        }
        checkDone();
      };
    });
  }

  /**
   */
  setMessageCallback(cb: (msg: types.VmMsg) => void): void {
    while (true) {
      const msg = this.#buf.shift();
      if (msg) {
        cb(msg);
      } else {
        break;
      }
    }
    this.#msgCb = cb;
  }

  /**
   */
  getHash(): types.VmHash {
    return this.#hash;
  }
}

/**
 */
export class VmHttpClient {
  #url: URL;
  #sign: types.VmMultiSign;
  #token: null | types.VmHash;
  #shortCache: null | types.VmObjSignedShortCache;
  #contextAccess: { [key: string]: [types.VmHash, any] };

  /**
   */
  constructor(url: URL, sign: types.VmMultiSign) {
    this.#url = url;
    this.#sign = sign;
    this.#token = null;
    this.#shortCache = null;
    this.#contextAccess = {};
  }

  /**
   */
  setApiToken(token: types.VmHash) {
    this.#token = token;
  }

  /**
   */
  getApiToken(): null | types.VmHash {
    return this.#token;
  }

  /**
   */
  setShortCache(shortCache: types.VmObjSignedShortCache) {
    if (this.#shortCache) {
      this.#shortCache = shortCache;
    }
  }

  /**
   */
  setAppAuthData(ctx: types.VmHash, app: any) {
    this.#contextAccess[ctx.toString()] = [ctx, app];
  }

  /**
   */
  async getAuthChalReq(): Promise<types.VmAuthChalReq> {
    this.#url.pathname = "/auth-chal-req";
    const res = await fetch(this.#url);
    if (res.status >= 400) {
      const msg = await res.text();
      throw new Error(`error(${res.status}): ${msg}`);
    }
    const parsed = unpack(new Uint8Array(await res.arrayBuffer()));
    if (
      typeof parsed === "object" &&
      parsed.token instanceof Uint8Array &&
      parsed.nonce instanceof Uint8Array
    ) {
      return {
        token: new types.VmHash(parsed.token),
        nonce: new types.VmHash(parsed.nonce),
      };
    }
    throw new TypeError("invalid response type");
  }

  /**
   */
  async putAuthChalRes(
    token: types.VmHash,
    response: types.VmAuthChalRes,
  ): Promise<void> {
    this.#url.pathname = "/auth-chal-res";
    const nonceSig = [];
    for (const { pk, sig } of response.nonceSig) {
      nonceSig.push({ pk: pk.encode(), sig: sig.encode() });
    }
    const contextAccess = [];
    for (const [ctx, value] of response.contextAccess) {
      contextAccess.push([ctx.data(), value]);
    }
    const res = await fetch(this.#url, {
      body: pack({
        nonceSig,
        contextAccess,
      }),
      headers: {
        Authorization: `Bearer ${token.toString()}`,
      },
      method: "PUT",
    } as RequestInit);
    if (res.status >= 400) {
      const msg = await res.text();
      throw new Error(`error(${res.status}): ${msg}`);
    }
  }

  private async authenticate(): Promise<void> {
    const req = await this.getAuthChalReq();

    const contextAccess = [];

    for (const key in this.#contextAccess) {
      contextAccess.push(this.#contextAccess[key]);
    }

    let res: types.VmAuthChalRes = {
      nonceSig: this.#sign.sign(req.nonce.data()),
      contextAccess,
    };

    await this.putAuthChalRes(req.token, res);

    this.#token = req.token;
  }

  private async retryAuth<T>(cb: () => Promise<T>): Promise<T> {
    if (!this.#token) {
      await this.authenticate();
    }
    try {
      return await cb();
    } catch (_e) {
      await this.authenticate();
      return await cb();
    }
  }

  /**
   */
  async listen(): Promise<VmWebSocket> {
    return await this.retryAuth(async () => {
      const url = new URL(this.#url);
      if (url.protocol === "https:") {
        url.protocol = "wss";
      } else if (url.protocol === "http:") {
        url.protocol = "ws";
      }
      url.pathname = `/listen/${this.#token?.toString()}`;
      return await VmWebSocket.connect(url);
    });
  }

  /**
   */
  async send(
    ctx: types.VmHash,
    peerHash: types.VmHash,
    data: Uint8Array,
  ): Promise<void> {
    return await this.retryAuth(async () => {
      this.#url.pathname = `/send/${ctx.toString()}/${peerHash.toString()}`;
      const res = await fetch(this.#url, {
        body: data,
        headers: {
          Authorization: `Bearer ${this.#token?.toString()}`,
        },
        method: "PUT",
      } as RequestInit);
      if (res.status >= 400) {
        const msg = await res.text();
        throw new Error(`error(${res.status}): ${msg}`);
      }
    });
  }

  /**
   */
  async context(
    ctx: types.VmHash,
    config: types.VmContextConfig,
  ): Promise<void> {
    return await this.retryAuth(async () => {
      this.#url.pathname = `/context/${ctx.toString()}`;
      const res = await fetch(this.#url, {
        body: config.encode(),
        headers: {
          Authorization: `Bearer ${this.#token?.toString()}`,
        },
        method: "PUT",
      } as RequestInit);
      if (res.status >= 400) {
        const msg = await res.text();
        throw new Error(`error(${res.status}): ${msg}`);
      }
    });
  }

  /**
   */
  async insert(ctx: types.VmHash, data: Uint8Array): Promise<void> {
    return await this.retryAuth(async () => {
      this.#url.pathname = `/insert/${ctx.toString()}`;
      const res = await fetch(this.#url, {
        body: data,
        headers: {
          Authorization: `Bearer ${this.#token?.toString()}`,
        },
        method: "PUT",
      } as RequestInit);
      if (res.status >= 400) {
        const msg = await res.text();
        throw new Error(`error(${res.status}): ${msg}`);
      }
    });
  }

  /**
   */
  async select(ctx: types.VmHash, data: Uint8Array): Promise<Uint8Array> {
    return await this.retryAuth(async () => {
      this.#url.pathname = `/select/${ctx.toString()}`;
      const res = await fetch(this.#url, {
        body: data,
        headers: {
          Authorization: `Bearer ${this.#token?.toString()}`,
        },
        method: "PUT",
      } as RequestInit);
      if (res.status >= 400) {
        const msg = await res.text();
        throw new Error(`error(${res.status}): ${msg}`);
      }
      return new Uint8Array(await res.arrayBuffer());
    });
  }
}
