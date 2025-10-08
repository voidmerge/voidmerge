export { ObjMeta } from "./obj-meta.js";
import { ObjMeta } from "./obj-meta.js";

type VmRawReq =
  | {
      type: "objCheckReq";
      data: Uint8Array;
      meta: string;
    }
  | {
      type: "fnReq";
      method: string;
      path: string;
      body: Uint8Array;
      headers: { [header: string]: string };
    };

// define types / functions expected and provided by the vm system
declare global {
  var vm: (req: VmRawReq) => Promise<
    | {
        type: "objCheckResOk";
      }
    | {
        type: "fnResOk";
      }
  >;
  function objPut(
    data: Uint8Array,
    meta: {
      appPath?: string;
      expiresSecs?: number;
    },
  ): Promise<string>;
  function objList(
    appPathPrefix: string,
    createdGt: number,
    limit: number,
  ): Promise<string[]>;
  function objGet(meta: string): Promise<{ meta: string; data: Uint8Array }>;
}

/**
 * Incoming request to validate object data for storage.
 * - If this data is not valid, throw an exception.
 * - If this data is valid, return {@link RequestObjCheck.okResponse}.
 */
export class RequestObjCheck {
  #data: Uint8Array;
  #meta: ObjMeta;

  /**
   * Construct a new ObjCheck request instance.
   */
  constructor(data: Uint8Array, meta: ObjMeta) {
    this.#data = data;
    this.#meta = meta;
  }

  /**
   * Instance type marker.
   */
  static type(): string {
    return "objCheckReq";
  }

  /**
   * Instance type marker.
   */
  type(): string {
    return "objCheckReq";
  }

  /**
   * Generate an ok/success response.
   */
  static okResponse(): ResponseObjCheckOk {
    return RES_OBJ_CHECK_OK;
  }

  /**
   * Get the data of the object to be stored.
   */
  data(): Uint8Array {
    return this.#data;
  }

  /**
   * Get the meta info of the object to be stored.
   */
  meta(): ObjMeta {
    return this.#meta;
  }
}

const RES_OBJ_CHECK_OK: ResponseObjCheckOk = Object.freeze({
  type: "objCheckResOk",
});

/**
 * Incoming function handler request.
 * - If the request is malformed or errors, throw an exception.
 * - If the request succeeds, return {@link RequestFn.okResponse}.
 */
export class RequestFn {
  #method: string;
  #path: string;
  #headers: { [header: string]: string };
  #body?: Uint8Array;

  /**
   * Construct a new function request.
   */
  constructor(
    method: string,
    path: string,
    headers: { [header: string]: string },
    body?: Uint8Array,
  ) {
    this.#method = method;
    this.#path = path;
    this.#body = body;
    this.#headers = headers;
  }

  /**
   * Instance type marker.
   */
  static type(): string {
    return "fnReq";
  }

  /**
   * Instance type marker.
   */
  type(): string {
    return "fnReq";
  }

  /**
   * Generate an ok/success response.
   */
  static okResponse(
    status: number,
    body: Uint8Array,
    headers?: { [header: string]: string },
  ): ResponseFnOk {
    return {
      type: "fnResOk",
      status,
      body,
      headers: headers || {},
    };
  }

  /**
   * Get the method of the request ("GET", or "PUT").
   */
  method(): string {
    return this.#method;
  }

  /**
   * Get the path of the request (will not include the context).
   */
  path(): string {
    return this.#path;
  }

  /**
   * Get any headers passed in along with the request.
   */
  headers(): { [header: string]: string } {
    return this.#headers;
  }

  /**
   * If this was a "PUT" request, get the passed in body.
   */
  body(): Uint8Array {
    if (this.#body) {
      return this.#body;
    } else {
      return EMPTY;
    }
  }
}

const EMPTY: Uint8Array = new Uint8Array(0);

/**
 * Union of request types.
 */
export type Request = RequestObjCheck | RequestFn;

/**
 * Success response type for an ObjCheck request.
 */
export interface ResponseObjCheckOk {
  type: "objCheckResOk";
}

/**
 * Success response type for a function request.
 */
export interface ResponseFnOk {
  type: "fnResOk";
  status: number;
  body: Uint8Array;
  headers: { [header: string]: string };
}

/**
 * Union of response types.
 */
export type Response = ResponseObjCheckOk | ResponseFnOk;

/**
 * Function signature for a VoidMerge handler.
 */
export type VoidMergeHandler = (request: Request) => Promise<Response>;

/**
 * Define a single global VoidMerge handler function.
 * Will error if called multiple times or if for any other reason a
 * vm handler function already exists.
 */
export function defineVoidMergeHandler(handler: VoidMergeHandler) {
  if ("vm" in globalThis) {
    throw new Error(
      "global 'vm' function already defined, you can only define a single handler.",
    );
  }

  // Define the handler with some translations for typescript type instances.
  globalThis.vm = async (req: VmRawReq) => {
    const type = req.type;
    if (req.type === "objCheckReq") {
      return await handler(
        new RequestObjCheck(req.data, new ObjMeta(req.meta)),
      );
    } else if (req.type === "fnReq") {
      return await handler(
        new RequestFn(req.method, req.path, req.headers, req.body),
      );
    }
    throw new Error(`invalid request type: ${type}`);
  };
}

/**
 * Put some data in the object store. Returns the finalized meta path.
 */
export async function objPut(
  data: Uint8Array,
  meta: {
    appPath: string;
    expiresSecs?: number;
  },
): Promise<ObjMeta> {
  const metaOut = await globalThis.objPut(data, meta);
  return new ObjMeta(metaOut);
}

/**
 * Callback function signature for {@link objList}.
 */
export type ObjListPager = (meta: ObjMeta[]) => Promise<void>;

/**
 * List data from the object store.
 */
export async function objList(
  appPathPrefix: string,
  createdGt: number,
  limit: number,
): Promise<ObjMeta[]> {
  const out = [];
  for (const path of await globalThis.objList(
    appPathPrefix,
    createdGt,
    limit,
  )) {
    out.push(new ObjMeta(path));
  }
  return out;
}

/**
 * Get an object from the object store given a finalized meta path.
 */
export async function objGet(
  meta: ObjMeta,
): Promise<{ meta: ObjMeta; data: Uint8Array }> {
  const { meta: m, data } = await globalThis.objGet(meta.fullPath());
  return { meta: new ObjMeta(m), data };
}
