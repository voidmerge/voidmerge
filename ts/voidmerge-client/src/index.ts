import fetch from "cross-fetch";
import { unpack } from "msgpackr/unpack";
//import { pack } from "msgpackr/pack";

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
): Promise<Uint8Array> {
  const getUrl = new URL(url);
  getUrl.pathname = path;
  if (query) {
    for (const k in query) {
      getUrl.searchParams.set(k, query[k]);
    }
  }
  const opts: RequestInit = {};
  if (token) {
    opts.headers = {
      Authorization: `Bearer ${token}`,
    };
  }
  return await handle(await fetch(getUrl, opts));
}

async function put(
  url: URL | string,
  path: string,
  token: string,
  body: Uint8Array,
): Promise<Uint8Array> {
  const putUrl = new URL(url);
  putUrl.pathname = path;

  const res = await fetch(putUrl, {
    body,
    headers: {
      Authorization: `Bearer ${token}`,
    },
    method: "PUT",
  } as RequestInit);

  return await handle(res);
}

/**
 * Execute a simple health check on a VoidMerge server.
 */
export async function health(url: URL | string) {
  await get(url, "");
}

/**
 * Put an object into a VoidMerge server context store.
 */
export async function objPut(input: {
  url: URL | string;
  token: string;
  ctx: string;
  appPath: string;
  createdSecs?: string | number;
  expiresSecs?: string | number;
  data: Uint8Array;
}): Promise<{ meta: string }> {
  const { url, token, ctx, appPath, createdSecs, expiresSecs, data } = input;
  const cs = createdSecs ? createdSecs.toString() : "0";
  const es = expiresSecs ? expiresSecs.toString() : "0";
  const meta = new TextDecoder().decode(
    await put(url, `${ctx}/_vm_/obj-put/${appPath}/${cs}/${es}`, token, data),
  );
  return { meta };
}

/**
 * List object metadata in a VoidMerge server context store.
 */
export async function objList(input: {
  url: URL | string;
  token: string;
  ctx: string;
  appPathPrefix: string;
  createdGt?: string | number;
  limit?: string | number;
}): Promise<{ metaList: string[] }> {
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
export async function objGet(input: {
  url: URL | string;
  token: string;
  ctx: string;
  appPath: string;
}): Promise<{ meta: string; data: Uint8Array }> {
  const { url, token, ctx, appPath } = input;
  const res = await get(
    url,
    `${ctx}/_vm_/obj-get/${appPath}`,
    undefined,
    token,
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

/*
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
      return new Uint8Array(await res.arrayBuffer());
*/
