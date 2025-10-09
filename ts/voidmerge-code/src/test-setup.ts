const CTX: string = "AAAA";
const STORE: { [meta: string]: Uint8Array } = {};

export function checkTestSetup() {
  if (!("VM" in globalThis)) {
    globalThis.VM = {
      ctx(): string {
        return CTX;
      },
      async objPut(input: {
        meta: string;
        data: Uint8Array;
      }): Promise<{ meta: string }> {
        STORE[input.meta] = input.data;
        return { meta: input.meta };
      },
      async objList(input: {
        appPathPrefix: string;
        createdGt: number;
        limit: number;
      }): Promise<{ metaList: string[] }> {
        return { metaList: Object.keys(STORE) };
      },
      async objGet(input: {
        meta: string;
      }): Promise<{ meta: string; data: Uint8Array }> {
        if (input.meta in STORE) {
          return { meta: input.meta, data: STORE[input.meta] };
        }
        throw new Error("not found");
      },
    };
  }
}
