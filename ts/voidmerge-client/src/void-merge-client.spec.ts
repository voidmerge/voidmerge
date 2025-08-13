import * as types from "./types";
import { VmSignP256 } from "./sign-p256";
import { Vm } from "./vm-test-helper";
import { VoidMergeClient } from "./void-merge-client";
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

describe("VoidMergeClient", () => {
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

  it("insert/select with sysadmin", async () => {
    const sign = new types.VmMultiSign();
    sign.addSign(new VmSignP256());

    const client = new VoidMergeClient(
      sign,
      new URL(`http://127.0.0.1:${test.vm?.port()}`),
    );

    client.setApiToken(types.VmHash.parse("bobo"));
    client.setShortCache(new types.VmObjSignedShortCacheLru(4096));

    const ctx = types.VmHash.nonce();

    await client.insert(
      ctx,
      new types.VmObj("syslogic")
        .withIdent(types.VmHash.parse("AAAA"))
        .withApp(LOGIC),
    );

    await client.insert(
      ctx,
      new types.VmObj("test").withApp({ test: "apple" }),
    );

    const results = await client.select(
      ctx,
      new types.VmSelect().withFilterByTypes(["test"]).withReturnData(true),
    );

    expect(results.count).toEqual(1);
    if (!Array.isArray(results.results)) {
      throw new Error("no results");
    }
    expect(results.results.length).toEqual(1);
    const result = results.results[0];
    if (!result.data) {
      throw new Error("no data returned");
    }
    if (!result.data.parsed.app) {
      throw new Error("no app payload returned");
    }
    expect(result.data.parsed.app).toEqual({ test: "apple" });
  });

  it("insert/select with auth", async () => {
    const sign = new types.VmMultiSign();
    sign.addSign(new VmSignP256());

    const adminClient = new VoidMergeClient(
      sign,
      new URL(`http://127.0.0.1:${test.vm?.port()}`),
    );

    adminClient.setApiToken(types.VmHash.parse("bobo"));
    adminClient.setShortCache(new types.VmObjSignedShortCacheLru(4096));

    const ctx = types.VmHash.nonce();

    await adminClient.insert(
      ctx,
      new types.VmObj("syslogic")
        .withIdent(types.VmHash.parse("AAAA"))
        .withApp(LOGIC),
    );

    const client = new VoidMergeClient(
      sign,
      new URL(`http://127.0.0.1:${test.vm?.port()}`),
    );

    client.setShortCache(new types.VmObjSignedShortCacheLru(4096));
    client.setAppAuthData(ctx, null);

    await client.insert(
      ctx,
      new types.VmObj("test").withApp({ test: "apple" }),
    );

    const results = await client.select(
      ctx,
      new types.VmSelect().withFilterByTypes(["test"]).withReturnData(true),
    );

    expect(results.count).toEqual(1);
    if (!Array.isArray(results.results)) {
      throw new Error("no results");
    }
    expect(results.results.length).toEqual(1);
    const result = results.results[0];
    if (!result.data) {
      throw new Error("no data returned");
    }
    if (!result.data.parsed.app) {
      throw new Error("no app payload returned");
    }
    expect(result.data.parsed.app).toEqual({ test: "apple" });
  });

  it("WebSocket", async () => {
    const sign = new types.VmMultiSign();
    sign.addSign(new VmSignP256());

    const client = new VoidMergeClient(
      sign,
      new URL(`http://127.0.0.1:${test.vm?.port()}`),
    );

    client.setApiToken(types.VmHash.parse("bobo"));
    client.setShortCache(new types.VmObjSignedShortCacheLru(4096));

    const peerHash = await client.getThisPeerHash();

    const res = await new Promise((res, rej) => {
      const timer = setTimeout(() => rej("timeout awaiting ws msg"), 5000);
      client.setMessageCallback((msg: types.VmMsg) => {
        clearTimeout(timer);
        res(new TextDecoder().decode(msg.data));
      });

      client
        .send(types.VmHash.nonce(), peerHash, new TextEncoder().encode("hello"))
        .then(
          () => {},
          (err) => {
            clearTimeout(timer);
            rej(err);
          },
        );
    });

    expect(res).toEqual("hello");
  });
});
