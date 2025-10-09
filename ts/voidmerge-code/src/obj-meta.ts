/**
 * Object store metadata/path.
 *
 * This represents the storage location of some data in an object store,
 * including some metadata encoded in that path.
 */
export class ObjMeta {
  #fullPath: string;
  #sysPrefix: string;
  #ctx: string;
  #appPath: string;
  #createdSecs: number;
  #expiresSecs: number;
  #byteLength: number;

  private constructor(fullPath: string) {
    const parts = fullPath.split("/");
    this.#sysPrefix = "c";
    this.#ctx = globalThis.VM.ctx();
    this.#appPath = parts[2] || "";
    this.#createdSecs = parseFloat(parts[3] || "0");
    this.#expiresSecs = parseFloat(parts[4] || "0");
    this.#byteLength = parseFloat(parts[5] || "0");
    this.#fullPath = `${this.#sysPrefix}/${this.#ctx}/${this.#appPath}/${this.#createdSecs}/${this.#expiresSecs}/${this.#byteLength}`;
  }

  /**
   * Validate and construct an ObjMeta from a full path string.
   */
  static fromFull(path: string): ObjMeta {
    return new ObjMeta(path);
  }

  /**
   * Generate an ObjMeta from components, filling in stubs for unknowns.
   */
  static fromParts(input: {
    appPath: string;
    createdSecs?: string | number;
    expiresSecs?: string | number;
    byteLength?: string | number;
  }): ObjMeta {
    const { appPath, createdSecs, expiresSecs, byteLength } = input;
    const ctx = globalThis.VM.ctx();
    const cs: string = createdSecs ? createdSecs.toString() : "0";
    const es: string = expiresSecs ? expiresSecs.toString() : "0";
    const bl: string = byteLength ? byteLength.toString() : "0";
    return new ObjMeta(`c/${ctx}/${appPath}/${cs}/${es}/${bl}`);
  }

  /**
   * Get the full ObjMeta path.
   */
  fullPath(): string {
    return this.#fullPath;
  }

  /**
   * Get the system prefix component of the ObjMeta path.
   */
  sysPrefix(): string {
    return this.#sysPrefix;
  }

  /**
   * Get the context identifier component of the ObjMeta path.
   */
  ctx(): string {
    return this.#ctx;
  }

  /**
   * Get the appPath component of the ObjMeta path.
   */
  appPath(): string {
    return this.#appPath;
  }

  /**
   * Get the createdSecs timestamp component of the ObjMeta path.
   */
  createdSecs(): number {
    return this.#createdSecs;
  }

  /**
   * Get the expiresSecs timestamp component of the ObjMeta path.
   */
  expiresSecs(): number {
    return this.#expiresSecs;
  }

  /**
   * Get the byte length of the data associated with this ObjMeta path.
   */
  byteLength(): number {
    return this.#byteLength;
  }
}
