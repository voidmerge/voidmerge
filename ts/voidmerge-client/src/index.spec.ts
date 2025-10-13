import { VmNodeTestServer } from "./vm-node-test-server.js";
import * as VM from "./index.js";

/// since we use `cargo run` it may take a while building code
const TIMEOUT = 30000;

describe("voidmerge-client", () => {
  const test: { vm: null | VmNodeTestServer; url: string } = {
    vm: null,
    url: "http://127.0.0.1:0",
  };

  const teardown = async () => {
    if (!test.vm) {
      return;
    }
    await test.vm.kill();
    test.vm = null;
  };

  const setup = async (bundle: string) => {
    await teardown();
    test.vm = await VmNodeTestServer.spawn(
      `ts/test-integration/dist/bundle-${bundle}.js`,
    );
    test.url = `http://127.0.0.1:${test.vm?.port()}`;
  };

  afterEach(async () => {
    await teardown();
  });

  it(
    "health",
    async () => {
      await setup("obj-simple");
      await VM.health(test.url);
    },
    TIMEOUT,
  );

  it(
    "simple put,list,get",
    async () => {
      await setup("obj-simple");

      const { meta } = await VM.objPut({
        url: test.url,
        token: "test",
        ctx: "test",
        appPath: "bob",
        data: new TextEncoder().encode("hello"),
      });

      const { metaList } = await VM.objList({
        url: test.url,
        token: "test",
        ctx: "test",
        appPathPrefix: "b",
      });

      expect(metaList).toEqual([meta]);

      const { meta: meta2, data } = await VM.objGet({
        url: test.url,
        token: "test",
        ctx: "test",
        appPath: "bob",
      });

      expect(meta2).toEqual(meta);
      expect(new TextDecoder().decode(data)).toEqual("hello");
    },
    TIMEOUT,
  );

  it(
    "simple msg",
    async () => {
      await setup("msg-simple");

      const { body } = await VM.fnCall({
        url: test.url,
        ctx: "test",
        path: "listen",
      });

      const msgId = new TextDecoder().decode(body);

      let res: null | ((r: any) => void) = null;
      let rej: null | ((r: any) => void) = null;
      let wait = new Promise((g, b) => {
        res = g;
        rej = b;
      });

      const listener = await VM.MsgListener.connect({
        url: test.url,
        ctx: "test",
        msgId,
        handler: (input) => {
          if (input.err) {
            if (rej) {
              rej(input.err);
            }
          } else {
            if (
              res &&
              input.msg &&
              input.msg.type === "app" &&
              input.msg.msg instanceof Uint8Array
            ) {
              res(new TextDecoder().decode(input.msg.msg));
            }
          }
        },
      });

      await VM.fnCall({
        url: test.url,
        ctx: "test",
        path: "sendall",
        body: new TextEncoder().encode("hello"),
      });

      const msg = await wait;

      await listener.close();

      expect(msg).toEqual("hello");
    },
    TIMEOUT,
  );
});
