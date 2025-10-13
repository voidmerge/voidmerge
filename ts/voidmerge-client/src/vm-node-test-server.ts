import { spawn, ChildProcess } from "node:child_process";
import { env } from "node:process";

export class VmNodeTestServer {
  #proc: ChildProcess;
  #port: number;

  constructor(proc: ChildProcess, port: number) {
    this.#proc = proc;
    this.#port = port;
  }

  static async spawn(codeFile: string): Promise<VmNodeTestServer> {
    const vm = await VmNodeTestServer.priv_spawn(codeFile);

    for (let i = 0; i < 40; ++i) {
      const url = new URL(`http://127.0.0.1:${vm.port()}/`);
      const res = await fetch(url);
      if (res.status === 200) {
        return vm;
      }

      let id = undefined;
      await new Promise((res, _rej) => {
        id = setTimeout(res, 100);
      });
      clearTimeout(id);
    }

    throw new Error("failed to spawn vm test server");
  }

  private static async priv_spawn(codeFile: string): Promise<VmNodeTestServer> {
    const { proc, port } = (await new Promise((res, rej) => {
      let pname = "vm";
      const pargs = [
        "test",
        "--http-addr",
        "127.0.0.1:0",
        "--code-file",
        codeFile,
      ];
      if (!env.CI) {
        pname = "cargo";
        pargs.unshift("--");
        pargs.unshift("rs/voidmerge/Cargo.toml");
        pargs.unshift("--manifest-path");
        pargs.unshift("run");
      }
      const proc = spawn(pname, pargs);
      proc.on("error", rej);
      let allbuf = new Uint8Array(0);
      const timer = setTimeout(() => rej("could not determine port"), 5000);
      proc.stderr.on("data", (data) => {
        const newA = new Uint8Array(allbuf.byteLength + data.byteLength);
        newA.set(allbuf);
        newA.set(data, allbuf.byteLength);
        allbuf = newA;
        const text = new TextDecoder().decode(allbuf);
        const find = text.match(/#vm#listening#127.0.0.1:(\d+)#/);
        if (find && find.length >= 2) {
          clearTimeout(timer);
          res({ proc, port: parseInt(find[1]) });
        }
      });
    })) as { proc: ChildProcess; port: number };
    return new VmNodeTestServer(proc, port);
  }

  port(): number {
    return this.#port;
  }

  async kill(): Promise<void> {
    this.#proc.kill("SIGKILL");
  }
}
