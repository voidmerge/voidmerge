import { VmNodeTestServer } from "./vm-node-test-server.js";
import { health, objPut, objList, objGet } from "./index.js";

describe("voidmerge-client", () => {
  const test: { vm: null | VmNodeTestServer; url: string } = {
    vm: null,
    url: "http://127.0.0.1:0",
  };

  beforeEach(async () => {
    if (test.vm !== null) {
      throw new Error("concurrent test problem");
    }
    test.vm = await VmNodeTestServer.spawn(
      "ts/test-integration/dist/bundle-obj-simple.js",
    );
    test.url = `http://127.0.0.1:${test.vm?.port()}`;
  });

  afterEach(async () => {
    if (!test.vm) {
      throw new Error("concurrent test problem");
    }
    await test.vm.kill();
    test.vm = null;
  });

  it("health", async () => {
    await health(test.url);
  });

  it("simple put,list,get", async () => {
    const { meta } = await objPut({
      url: test.url,
      token: "test",
      ctx: "test",
      appPath: "bob",
      data: new TextEncoder().encode("hello"),
    });

    const { metaList } = await objList({
      url: test.url,
      token: "test",
      ctx: "test",
      appPathPrefix: "b",
    });

    expect(metaList).toEqual([meta]);

    const { meta: meta2, data } = await objGet({
      url: test.url,
      token: "test",
      ctx: "test",
      appPath: "bob",
    });

    expect(meta2).toEqual(meta);
    expect(new TextDecoder().decode(data)).toEqual("hello");
  });
});

/*
import * as types from "./types.js";
import { VmSignP256 } from "./sign-p256.js";
import { VmHttpClient } from "./http-client.js";
import { VmNodeTestServer } from "./vm-node-test-server.js";
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
  const test: { vm: null | VmNodeTestServer } = { vm: null };

  beforeEach(async () => {
    if (test.vm !== null) {
      throw new Error("concurrent test problem");
    }
    test.vm = await VmNodeTestServer.spawn();
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

    await c.context(ctx, new types.VmContextConfig());

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
*/
