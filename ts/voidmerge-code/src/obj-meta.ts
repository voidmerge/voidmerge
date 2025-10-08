/**
 * Object store metadata/path.
 *
 * This represents the storage location of some data in an object store,
 * including some metadata encoded in that path.
 */
export class ObjMeta {
  #path: string;
  #parts: string[];

  private constructor(path: string) {
    this.#path = path;
    this.#parts = this.#path.split("/");
  }

  /**
   * Validate and construct an ObjMeta from a full path string.
   */
  static fromFull(path: string): ObjMeta {
    const out = new ObjMeta(path);
    if (out.sysPrefix() !== "c") {
      throw new Error(`invalid sysPrefix: '${out.sysPrefix()}', expected 'c'`);
    }
    const ctx = globalThis.VM.ctx();
    if (out.ctx() !== ctx) {
      throw new Error(`invalid ctx: '${out.ctx()}', expected '${ctx}'`);
    }
    return out;
  }

  /**
   * Generate an ObjMeta from components, filling in stubs for unknowns.
   */
  static fromParts(appPath: string, expiresSecs?: number): ObjMeta {
    const ctx = globalThis.VM.ctx();
    return ObjMeta.fromFull(`c/${ctx}/${appPath}/0/${expiresSecs || 0}/0`);
  }

  /**
   * Get the full ObjMeta path.
   */
  fullPath(): string {
    return this.#path;
  }

  /**
   * Get the system prefix component of the ObjMeta path.
   */
  sysPrefix(): string {
    if (
      this.#parts[0] === "s" ||
      this.#parts[0] == "x" ||
      this.#parts[0] == "d" ||
      this.#parts[0] == "c"
    ) {
      return this.#parts[0];
    }
    throw new Error(`invalid ObjMeta sys_prefix: ${this.#parts[0]}`);
  }

  /**
   * Get the context identifier component of the ObjMeta path.
   */
  ctx(): string {
    if (this.#parts.length >= 2) {
      return this.#parts[1];
    }
    throw new Error(`invalid ObjMeta path, no context`);
  }

  /**
   * Get the appPath component of the ObjMeta path.
   */
  appPath(): string {
    if (this.#parts.length >= 3) {
      return this.#parts[2];
    }
    throw new Error(`invalid ObjMeta path, no appPath component`);
  }

  /**
   * Get the createdSecs timestamp component of the ObjMeta path.
   */
  createdSecs(): number {
    if (this.#parts.length >= 4) {
      return parseFloat(this.#parts[3]);
    }
    throw new Error(`invalid ObjMeta path, no createdSecs component`);
  }

  /**
   * Get the expiresSecs timestamp component of the ObjMeta path.
   */
  expiresSecs(): number {
    if (this.#parts.length >= 5) {
      return parseFloat(this.#parts[4]);
    }
    throw new Error(`invalid ObjMeta path, no expiresSecs component`);
  }

  /**
   * Get the byte length of the data associated with this ObjMeta path.
   */
  byteLength(): number {
    if (this.#parts.length >= 6) {
      return parseFloat(this.#parts[5]);
    }
    throw new Error(`invalid ObjMeta path, no byteLength component`);
  }
}
