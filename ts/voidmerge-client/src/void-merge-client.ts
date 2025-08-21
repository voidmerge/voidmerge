import * as types from "./types";
import * as http from "./http-client";

/**
 * A VoidMergeClient can talk to VoidMerge servers.
 */
export class VoidMergeClient {
  #multiSign: types.VmMultiSign;
  #client: http.VmHttpClient;
  #context: types.VmHash;
  #msgCb: null | ((msg: types.VmMsg) => void);
  #ws: null | http.VmWebSocket;
  #didAuth: boolean;

  /**
   * Construct a new VoidMergeClient.
   */
  constructor(
    multiSign: types.VmMultiSign,
    serverUrl: URL,
    context: types.VmHash,
  ) {
    this.#multiSign = multiSign;
    this.#client = new http.VmHttpClient(serverUrl, multiSign);
    this.#context = context;
    this.#client.setAppAuthData(this.#context, null);
    this.#msgCb = null;
    this.#ws = null;
    this.#didAuth = false;
  }

  private async checkAuth(): Promise<void> {
    if (this.#didAuth) {
      return;
    }

    // TODO auth_req/res

    if (!this.#ws) {
      this.#ws = await this.#client.listen();
      if (this.#msgCb) {
        this.#ws.setMessageCallback(this.#msgCb);
      }
    }

    this.#didAuth = true;
  }

  /**
   * If you have an invite or admin api token, specify it here.
   */
  setApiToken(token: types.VmHash) {
    this.#client.setApiToken(token);
  }

  /**
   * Set the ShortCache to use when fetching VmObjSigned data.
   *
   * It is recommended to use the same short cache across all clients
   * to ensure the memory usage is bounded for the entire application.
   */
  setShortCache(shortCache: types.VmObjSignedShortCache) {
    this.#client.setShortCache(shortCache);
  }

  /**
   */
  setAppAuthData(app: any) {
    this.#client.setAppAuthData(this.#context, app);
  }

  /**
   * Set a message callback for handling incoming message data.
   */
  setMessageCallback(cb: (msg: types.VmMsg) => void): void {
    this.#msgCb = cb;
    if (this.#ws) {
      this.#ws.setMessageCallback(cb);
    }
  }

  /**
   * Get this node's "peerHash" at which messages may be sent to this node.
   */
  async getThisPeerHash(): Promise<types.VmHash> {
    await this.checkAuth();
    if (!this.#ws) {
      throw new Error("failed to establish a listening connection");
    }
    return this.#ws.getHash();
  }

  /**
   * Send a message to a remote peer.
   */
  async send(peerHash: types.VmHash, data: Uint8Array): Promise<void> {
    await this.checkAuth();
    return await this.#client.send(this.#context, peerHash, data);
  }

  /**
   * Insert data into a VoidMerge server instance.
   */
  async insert(insert: types.VmObj): Promise<void> {
    await this.checkAuth();
    const data = insert.sign(this.#multiSign).encode();
    await this.#client.insert(this.#context, data);
  }

  /**
   */
  async select(select: types.VmSelect): Promise<types.VmSelectResponse> {
    await this.checkAuth();
    const data = select.encode();
    const res = await this.#client.select(this.#context, data);
    return types.VmSelectResponse.decode(res);
  }
}
