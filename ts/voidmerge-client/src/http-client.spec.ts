import * as types from "./types.js";
import { VmSignP256 } from "./sign-p256.js";
import { VmHttpClient } from "./http-client.js";
import { Vm } from "./vm-test-helper.js";
//import { unpack } from "msgpackr/unpack";
//import { pack } from "msgpackr/pack";

const LOGIC: types.VmLogicUtf8Single = {
  type: "utf8Single",
  code: `
VM({
  call:'register',
  code(i) {
    return { result: 'valid' };
  }
});
`,
};

describe("http-client", () => {
  const test: { vm: null | Vm } = { vm: null };

  beforeEach(async () => {
    if (test.vm !== null) {
      throw new Error("concurrent test problem");
    }
    test.vm = await Vm.spawn();
  });

  afterEach(async () => {
    if (!test.vm) {
      throw new Error("concurrent test problem");
    }
    await test.vm.kill();
    test.vm = null;
  });

  it("sanity", async () => {
    const sign = new types.VmMultiSign();
    sign.addSign(new VmSignP256());

    const c = new VmHttpClient(
      new URL(`http://127.0.0.1:${test.vm?.port()}`),
      sign,
    );
    c.setApiToken(types.VmHash.parse("bobo"));
    c.setShortCache(new types.VmObjSignedShortCacheLru(4096));

    const res = await c.getAuthChalReq();
    expect(res.token.data().byteLength).toEqual(24);

    const ctx = types.VmHash.nonce();

    const bundle = new types.VmObj("syslogic")
      .withIdent(types.VmHash.parse("AAAA"))
      .withApp(LOGIC)
      .sign(sign)
      .encode();

    await c.insert(ctx, bundle);
  });

  it("WebSocket", async () => {
    const sign = new types.VmMultiSign();
    sign.addSign(new VmSignP256());

    const c = new VmHttpClient(
      new URL(`http://127.0.0.1:${test.vm?.port()}`),
      sign,
    );
    c.setApiToken(types.VmHash.parse("bobo"));
    c.setShortCache(new types.VmObjSignedShortCacheLru(4096));

    const ctx = types.VmHash.nonce();

    const ws = await c.listen();

    const result = await new Promise((res, rej) => {
      const timer = setTimeout(() => rej("failed to get ws message"), 5000);
      ws.setMessageCallback((data) => {
        clearTimeout(timer);
        res(new TextDecoder().decode(data.data));
      });
      c.send(ctx, ws.getHash(), new TextEncoder().encode("hello")).then(
        () => {},
        (err) => {
          clearTimeout(timer);
          rej(err);
        },
      );
    });

    expect(result).toEqual("hello");
  });
});
