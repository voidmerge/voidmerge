export { ObjMeta } from "./obj-meta.js";

import { ObjMeta } from "./obj-meta.js";

type VmRawReq =
  | {
      type: "codeConfigReq";
    }
  | {
      type: "cronReq";
    }
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

interface GlobalVM {
  ctx(): string;
  env(): any;
  msgNew(): Promise<{ msgId: string }>;
  msgList(): Promise<{ msgIdList: string[] }>;
  msgSend(input: { msgId: string; msg: Uint8Array }): Promise<void>;
  objPut(input: { meta: string; data: Uint8Array }): Promise<{ meta: string }>;
  objList(input: {
    appPathPrefix: string;
    createdGt: number;
    limit: number;
  }): Promise<{ metaList: string[] }>;
  objGet(input: { meta: string }): Promise<{ meta: string; data: Uint8Array }>;
  objRm(input: { meta: string }): Promise<void>;
}

// define types / functions provided by the vm system
declare global {
  var vm: (req: VmRawReq) => Promise<
    | {
        type: "codeConfigResOk";
      }
    | {
        type: "cronResOk";
      }
    | {
        type: "objCheckResOk";
      }
    | {
        type: "fnResOk";
      }
  >;
  var VM: GlobalVM;
}

/**
 * Incoming request for code configuration.
 * - Respond with {@link ResponseCodeConfigOk}.
 */
export class RequestCodeConfig {
  /**
   * Type marker.
   */
  type: "codeConfigReq" = "codeConfigReq";

  /**
   * Construct a new request instance.
   */
  constructor() {
    Object.freeze(this);
  }
}

/**
 * Success response type for a codeConfig request.
 */
export class ResponseCodeConfigOk {
  type: "codeConfigResOk" = "codeConfigResOk";

  /**
   * How often to invoke the cron handler. Note that this
   * will be invoked on every running node... The code will
   * need to account for potential parallel invocations in
   * a cluster setup.
   */
  cronIntervalSecs?: number;

  /**
   * Construct a new instance.
   */
  constructor(input: { cronIntervalSecs?: number }) {
    this.cronIntervalSecs = input.cronIntervalSecs;
    Object.freeze(this);
  }
}

/**
 * Incoming request for cron execution.
 * - Respond with {@link ResponseCronOk}.
 */
export class RequestCron {
  /**
   * Type marker.
   */
  type: "cronReq" = "cronReq";

  /**
   * Construct a new request instance.
   */
  constructor() {
    Object.freeze(this);
  }
}

/**
 * Success response type for a codeConfig request.
 */
export class ResponseCronOk {
  type: "cronResOk" = "cronResOk";

  /**
   * Construct a new instance.
   */
  constructor() {
    Object.freeze(this);
  }
}

/**
 * Incoming request to validate object data for storage.
 * - If this data is not valid, throw an exception.
 * - If this data is valid, return {@link ResponseObjCheckOk}.
 */
export class RequestObjCheck {
  /**
   * Type marker.
   */
  type: "objCheckReq" = "objCheckReq";

  /**
   * The data to check.
   */
  data: Uint8Array;

  /**
   * Object metadata.
   */
  meta: ObjMeta;

  /**
   * Construct a new ObjCheck request instance.
   */
  constructor(input: { data: Uint8Array; meta: ObjMeta }) {
    this.data = input.data;
    this.meta = input.meta;
    Object.freeze(this);
  }
}

/**
 * Success response type for an ObjCheck request.
 */
export class ResponseObjCheckOk {
  type: "objCheckResOk" = "objCheckResOk";

  /**
   * Construct a new instance.
   */
  constructor() {
    Object.freeze(this);
  }
}

/**
 * Incoming function handler request.
 * - If the request is malformed or errors, throw an exception.
 * - If the request succeeds, return {@link ResponseFnOk}.
 */
export class RequestFn {
  /**
   * Type marker.
   */
  type: "fnReq" = "fnReq";

  /**
   * The method of the request ("GET", or "PUT").
   */
  method: string;

  /**
   * The path of the request (will not include the context).
   */
  path: string;

  /**
   * Any headers passed in along with the request.
   */
  headers: { [header: string]: string };

  /**
   * If this was a "PUT" request, the passed in body.
   */
  body: Uint8Array;

  /**
   * Construct a new function request.
   */
  constructor(input: {
    method: string;
    path: string;
    headers: { [header: string]: string };
    body?: Uint8Array;
  }) {
    this.method = input.method;
    this.path = input.path;
    this.headers = input.headers;
    this.body = input.body || new Uint8Array(0);
    Object.freeze(this);
  }
}

/**
 * Union of request types.
 */
export type Request =
  | RequestCodeConfig
  | RequestCron
  | RequestObjCheck
  | RequestFn;

/**
 * Success response type for a function request.
 */
export class ResponseFnOk {
  /**
   * Type marker.
   */
  type: "fnResOk" = "fnResOk";

  /**
   * Status code (i.e. 200).
   */
  status: number;

  /**
   * Response body.
   */
  body: Uint8Array;

  /**
   * Response headers.
   */
  headers: { [header: string]: string };

  /**
   * Construct a new FnOk response instance.
   */
  constructor(input: {
    status: number;
    body: Uint8Array;
    headers?: { [header: string]: string };
  }) {
    this.status = input.status;
    this.body = input.body;
    this.headers = input.headers || {};
    Object.freeze(this);
  }
}

/**
 * Union of response types.
 */
export type Response =
  | ResponseCodeConfigOk
  | ResponseCronOk
  | ResponseObjCheckOk
  | ResponseFnOk;

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
    if (req.type === "codeConfigReq") {
      return await handler(new RequestCodeConfig());
    } else if (req.type === "cronReq") {
      return await handler(new RequestCron());
    } else if (req.type === "objCheckReq") {
      return await handler(
        new RequestObjCheck({
          data: req.data,
          meta: ObjMeta.fromFull(req.meta),
        }),
      );
    } else if (req.type === "fnReq") {
      return await handler(
        new RequestFn({
          method: req.method,
          path: req.path,
          headers: req.headers,
          body: req.body,
        }),
      );
    }
    throw new Error(`invalid request type: ${type}`);
  };
}

/**
 * Get the current context under which this code is executing.
 */
export function ctx(): string {
  return globalThis.VM.ctx();
}

/**
 * Get any environment that was supplied with CtxConfig.
 */
export function env(): any {
  return globalThis.VM.env();
}

/**
 * Create a new message channel for communicating with clients.
 */
export async function msgNew(): Promise<{ msgId: string }> {
  return await globalThis.VM.msgNew();
}

/**
 * List the active (or pending) message channels.
 */
export async function msgList(): Promise<{ msgIdList: string[] }> {
  return await globalThis.VM.msgList();
}

/**
 * Send a message to the given message channel.
 * This message will be received with type 'app', as opposed to a
 * message sent by a peer client, which will receive type 'peer'.
 */
export async function msgSend(input: {
  msgId: string;
  msg: Uint8Array;
}): Promise<void> {
  return await globalThis.VM.msgSend(input);
}

/**
 * Put some data in the object store. Returns the finalized meta path.
 */
export async function objPut(input: {
  meta: ObjMeta;
  data: Uint8Array;
}): Promise<{ meta: ObjMeta }> {
  const { meta } = await globalThis.VM.objPut({
    meta: input.meta.fullPath(),
    data: input.data,
  });
  return { meta: ObjMeta.fromFull(meta) };
}

/**
 * List data from the object store.
 */
export async function objList(input: {
  appPathPrefix: string;
  createdGt: number;
  limit: number;
}): Promise<{ metaList: ObjMeta[] }> {
  const { metaList } = await globalThis.VM.objList(input);
  const metaListOut = [];
  for (const path of metaList) {
    metaListOut.push(ObjMeta.fromFull(path));
  }
  return { metaList: metaListOut };
}

/**
 * Get an object from the object store given a finalized meta path.
 */
export async function objGet(input: {
  meta: ObjMeta;
}): Promise<{ meta: ObjMeta; data: Uint8Array }> {
  const { meta, data } = await globalThis.VM.objGet({
    meta: input.meta.fullPath(),
  });
  return { meta: ObjMeta.fromFull(meta), data };
}

/**
 * Delete an object by path from the store.
 * Note, this is may not be compatible with sharding or backup/restore,
 * i.e. objects could become resurrected.
 * Consider tombstoning or otherwise ensure revalidation will fail.
 */
export async function objRm(input: { meta: ObjMeta }): Promise<void> {
  await globalThis.VM.objRm({
    meta: input.meta.fullPath(),
  });
}
