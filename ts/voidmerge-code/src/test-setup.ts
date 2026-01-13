const CTX: string = "AAAA";
const ENV: any = { envTest: "BBBB" };
const STORE: { [meta: string]: Uint8Array } = {};
const MSG: { [meta: string]: boolean } = {};

export function checkTestSetup() {
  if (!("VM" in globalThis)) {
    globalThis.VM = {
      ctx(): string {
        return CTX;
      },
      env(): any {
        return ENV;
      },
      async msgNew(): Promise<{ msgId: string }> {
        const msgId = Math.random().toString();
        MSG[msgId] = true;
        return { msgId };
      },
      async msgList(): Promise<{ msgIdList: string[] }> {
        return { msgIdList: Object.keys(MSG) };
      },
      async msgSend(input: { msgId: string; msg: Uint8Array }): Promise<void> {
        if (input.msgId in MSG) {
          return;
        }
        throw new Error("invalid msgId");
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
      async objRm(input: { meta: string }): Promise<void> {
        delete STORE[input.meta];
      },
    };
  }
}
