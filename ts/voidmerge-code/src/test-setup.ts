const CTX: string = "AAAA";
const STORE: { [meta: string]: Uint8Array } = {};

export function checkTestSetup() {
  if (!("VM" in globalThis)) {
    globalThis.VM = {
      ctx(): string {
        return CTX;
      },
      async objPut(
        data: Uint8Array,
        meta: {
          appPath?: string;
          expiresSecs?: number;
        },
      ): Promise<string> {
        const metaOut = `c/${CTX}/${meta.appPath || ""}/${Date.now() / 1000}/${meta.expiresSecs || 0}/0`;
        STORE[metaOut] = data;
        return metaOut;
      },
      async objList(
        appPathPrefix: string,
        createdGt: number,
        limit: number,
      ): Promise<string[]> {
        return Object.keys(STORE);
      },
      async objGet(meta: string): Promise<{ meta: string; data: Uint8Array }> {
        if (meta in STORE) {
          return { meta, data: STORE[meta] };
        }
        throw new Error("not found");
      },
    };
  }
}
