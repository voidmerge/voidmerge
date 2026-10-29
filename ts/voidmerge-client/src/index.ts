import { unpack } from "msgpackr/unpack";
//import { pack } from "msgpackr/pack";

/**
 * Default per-request timeout in milliseconds.
 */
const DEFAULT_TIMEOUT_MS = 30000;

/**
 * Optional per-request tuning shared by the client API calls.
 */
export interface RequestOptions {
  /**
   * Per-request timeout in milliseconds. Defaults to 30000.
   * A timeout aborts the request rather than hanging indefinitely.
   */
  timeoutMs?: number;

  /**
   * When true, send a `Connection: close` header so the connection is not
   * reused from a pool. This is a safeguard against stale keep-alive
   * connections on Node (browsers ignore this header and manage their own
   * connection pooling). Defaults to false.
   */
  connectionClose?: boolean;
}

/**
 * Fetch with a bounded per-request timeout. Uses `AbortSignal.timeout` when
 * available and falls back to an `AbortController` + timer otherwise.
 */
async function fetchWithTimeout(
  url: URL | string,
  opts: RequestInit,
  timeoutMs: number,
): Promise<Response> {
  if (typeof AbortSignal !== "undefined" && "timeout" in AbortSignal) {
    return await fetch(url, {
      ...opts,
      signal: AbortSignal.timeout(timeoutMs),
    });
  }

  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), timeoutMs);
  try {
    return await fetch(url, { ...opts, signal: controller.signal });
  } finally {
    clearTimeout(timer);
  }
}

async function handle(res: Response): Promise<Uint8Array> {
  if (res.status >= 400) {
    const msg = await res.text();
    throw new Error(`error(${res.status}): ${msg}`);
  }
  return new Uint8Array(await res.arrayBuffer());
}

async function get(
  url: URL | string,
  path: string,
  query?: { [k: string]: string },
  token?: string,
  opts?: RequestOptions,
): Promise<Uint8Array> {
  const getUrl = new URL(url);
  getUrl.pathname = path;
  if (query) {
    for (const k in query) {
      getUrl.searchParams.set(k, query[k]);
    }
  }
  const headers: { [k: string]: string } = {};
  if (token) {
    headers.Authorization = `Bearer ${token}`;
  }
  if (opts?.connectionClose) {
    headers.Connection = "close";
  }
  const init: RequestInit = {};
  if (Object.keys(headers).length > 0) {
    init.headers = headers;
  }
  return await handle(
    await fetchWithTimeout(getUrl, init, opts?.timeoutMs ?? DEFAULT_TIMEOUT_MS),
  );
}

async function put(
  url: URL | string,
  path: string,
  token: string,
  body: Uint8Array,
  opts?: RequestOptions,
): Promise<Uint8Array> {
  const putUrl = new URL(url);
  putUrl.pathname = path;

  const buf = body.slice(0, body.byteLength);

  const headers: { [k: string]: string } = {
    Authorization: `Bearer ${token}`,
  };
  if (opts?.connectionClose) {
    headers.Connection = "close";
  }

  const res = await fetchWithTimeout(
    putUrl,
    {
      body: buf,
      headers,
      method: "PUT",
    },
    opts?.timeoutMs ?? DEFAULT_TIMEOUT_MS,
  );

  return await handle(res);
}

/**
 * Execute a simple health check on a VoidMerge server.
 */
export async function health(url: URL | string, opts?: RequestOptions) {
  await get(url, "", undefined, undefined, opts);
}

/**
 * Execute a "RequestFn" call on a VoidMerge server.
 */
export async function fnCall(
  input: {
    url: URL | string;
    ctx: string;
    path: string;
    headers?: { [k: string]: string };
    body?: Uint8Array;
  } & RequestOptions,
): Promise<{
  status: number;
  body: Uint8Array;
  headers: { [k: string]: string };
}> {
  const {
    url,
    ctx,
    path,
    headers,
    body,
    timeoutMs = DEFAULT_TIMEOUT_MS,
    connectionClose = false,
  } = input;

  const fnUrl = new URL(url);
  fnUrl.pathname = `${ctx}/${path}`;

  const opts: RequestInit = {
    method: "GET",
  };

  if (body instanceof Uint8Array) {
    opts.method = "PUT";
    opts.body = body.slice().buffer;
  }

  const outHeaders: { [k: string]: string } = { ...headers };
  if (connectionClose) {
    outHeaders.Connection = "close";
  }
  if (Object.keys(outHeaders).length > 0) {
    opts.headers = outHeaders;
  }

  const res = await fetchWithTimeout(fnUrl, opts, timeoutMs);

  if (res.status >= 400) {
    const msg = await res.text();
    throw new Error(`error(${res.status}): ${msg}`);
  }

  const out: {
    status: number;
    body: Uint8Array;
    headers: { [k: string]: string };
  } = {
    status: res.status,
    headers: {},
    body: new Uint8Array(await res.arrayBuffer()),
  };

  for (const [k, v] of res.headers) {
    out.headers[k] = v;
  }

  return out;
}

/**
 * A VoidMerge app message.
 */
export interface MessageApp {
  /**
   * Message type.
   */
  type: "app";

  /**
   * Message payload.
   */
  msg: Uint8Array;
}

/**
 * A VoidMerge message.
 */
export type Message = MessageApp;

/**
 * A VoidMerge listening websocket connection
 */
export class MsgListener {
  #ws: WebSocket;

  private constructor(ws: WebSocket) {
    this.#ws = ws;
  }

  /**
   * Open a new msg listener websocket connection with the provided handler.
   */
  static async connect(input: {
    url: URL | string;
    ctx: string;
    msgId: string;
    handler: (input: { err?: Error; msg?: Message }) => void;
  }): Promise<MsgListener> {
    return await new Promise((res, rej) => {
      const { url, ctx, msgId, handler } = input;

      const listenUrl = new URL(url);
      listenUrl.pathname = `${ctx}/_vm_/msg-listen/${msgId}`;

      const ws = new WebSocket(listenUrl);
      ws.binaryType = "arraybuffer";

      const timer = setTimeout(() => {
        clearTimeout(timer);
        rej("timeout opening websocket");
      }, 10000);

      ws.onopen = () => {
        clearTimeout(timer);
        res(new MsgListener(ws));
      };

      ws.onclose = (evt: any) => {
        clearTimeout(timer);
        const reason = evt.reason || evt.toString();
        handler({ err: new Error(`closed: ${reason}`) });
      };

      ws.onerror = (err: any) => {
        clearTimeout(timer);
        err = err.message || err.type || err.toString();
        rej(new Error(`ws connect error: ${err}`));
        handler({ err: new Error(err) });
      };

      ws.onmessage = (evt: any) => {
        if (!evt || typeof evt !== "object") {
          return;
        }

        if (!evt.data) {
          return;
        }

        let buffer: Uint8Array = new Uint8Array(0);

        if (evt.data instanceof Uint8Array) {
          buffer = evt.data;
        } else if (evt.data instanceof ArrayBuffer) {
          buffer = new Uint8Array(evt.data);
        } else {
          return;
        }

        const raw = unpack(buffer);

        if (!raw || typeof raw !== "object" || typeof raw.type !== "string") {
          return;
        }

        if (raw.type === "app") {
          if (raw.msg instanceof Uint8Array) {
            handler({
              msg: {
                type: "app",
                msg: raw.msg,
              },
            });
          }
        }
      };
    });
  }

  /**
   * Close the listener.
   */
  async close(): Promise<void> {
    this.#ws.close();
  }
}

/**
 * Put an object into a VoidMerge server context store.
 */
export async function objPut(
  input: {
    url: URL | string;
    token: string;
    ctx: string;
    appPath: string;
    createdSecs?: string | number;
    expiresSecs?: string | number;
    data: Uint8Array;
  } & RequestOptions,
): Promise<{ meta: string }> {
  const { url, token, ctx, appPath, createdSecs, expiresSecs, data } = input;
  const cs = createdSecs ? createdSecs.toString() : "0";
  const es = expiresSecs ? expiresSecs.toString() : "0";
  const meta = new TextDecoder().decode(
    await put(
      url,
      `${ctx}/_vm_/obj-put/${appPath}/${cs}/${es}`,
      token,
      data,
      input,
    ),
  );
  return { meta };
}

/**
 * List object metadata in a VoidMerge server context store.
 */
export async function objList(
  input: {
    url: URL | string;
    token: string;
    ctx: string;
    appPathPrefix: string;
    createdGt?: string | number;
    limit?: string | number;
  } & RequestOptions,
): Promise<{ metaList: string[] }> {
  const { url, token, ctx, appPathPrefix, createdGt, limit } = input;
  const c = createdGt ? createdGt.toString() : "0";
  const l = limit ? limit.toString() : "1000";
  const res = await get(
    url,
    `${ctx}/_vm_/obj-list/${appPathPrefix}`,
    {
      createdGt: c,
      limit: l,
    },
    token,
    input,
  );
  const raw = unpack(res);
  if (!raw || typeof raw !== "object" || !Array.isArray(raw.metaList)) {
    throw new Error("invalid objList response type");
  }
  for (const item of raw.metaList) {
    if (typeof item !== "string") {
      throw new Error("invalid objList response type");
    }
  }
  return { metaList: raw.metaList };
}

/**
 * Get object data from a VoidMerge server context store.
 */
export async function objGet(
  input: {
    url: URL | string;
    token: string;
    ctx: string;
    appPath: string;
  } & RequestOptions,
): Promise<{ meta: string; data: Uint8Array }> {
  const { url, token, ctx, appPath } = input;
  const res = await get(
    url,
    `${ctx}/_vm_/obj-get/${appPath}`,
    undefined,
    token,
    input,
  );
  const raw = unpack(res);
  if (
    !raw ||
    typeof raw !== "object" ||
    typeof raw.meta !== "string" ||
    !(raw.data instanceof Uint8Array)
  ) {
    throw new Error("invalid objGet response type");
  }
  return raw;
}
