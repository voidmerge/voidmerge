import {
  createServer,
  type IncomingMessage,
  type Server,
  type ServerResponse,
} from "node:http";
import type { AddressInfo } from "node:net";
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

describe("voidmerge-client connection handling", () => {
  let server: null | Server = null;
  let url = "";

  const listen = async (
    handler: (req: IncomingMessage, res: ServerResponse) => void,
  ) => {
    server = createServer(handler);
    await new Promise<void>((res) =>
      server?.listen(0, "127.0.0.1", () => res()),
    );
    const addr = server.address() as AddressInfo;
    url = `http://127.0.0.1:${addr.port}`;
  };

  const close = async () => {
    if (!server) {
      return;
    }
    const s = server;
    server = null;
    await new Promise<void>((res) => {
      s.close(() => res());
      s.closeAllConnections();
    });
  };

  afterEach(async () => {
    await close();
  });

  it("times out on an unresponsive server", async () => {
    await listen(() => {
      // intentionally never respond
    });

    await expect(VM.health(url, { timeoutMs: 200 })).rejects.toBeDefined();
  });

  it("sends Connection: close when requested", async () => {
    let seen: string | undefined = undefined;
    await listen((req, res) => {
      seen = req.headers.connection;
      res.writeHead(200).end();
    });

    await VM.health(url, { connectionClose: true });

    expect(seen).toEqual("close");
  });

  it("recovers after the server closes an idle keep-alive connection", async () => {
    await listen((_req, res) => {
      res.writeHead(200).end();
      // drop the connection shortly after responding, simulating a server
      // closing an idle keep-alive socket
      setTimeout(() => res.socket?.destroy(), 50);
    });

    await VM.health(url);
    await new Promise((res) => setTimeout(res, 150));
    await VM.health(url);
  });
});
