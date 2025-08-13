import { spawn, ChildProcess } from "node:child_process";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { rm } from "node:fs/promises";
import * as mktemp from "mktemp";

export class Vm {
  #dir: string;
  #proc: ChildProcess;
  #port: number;

  constructor(dir: string, proc: ChildProcess, port: number) {
    this.#dir = dir;
    this.#proc = proc;
    this.#port = port;
  }

  static async spawn(): Promise<Vm> {
    const dir = await mktemp.createDir(join(tmpdir(), ".tmpXXXXXXXX"));
    if (!dir) {
      throw new Error("failed to get tempdir");
    }

    const { proc, port } = (await new Promise((res, rej) => {
      const env = JSON.parse(JSON.stringify(process.env));
      env.VM_SYSADMIN_TOKENS = "bobo";
      env.VM_HTTP_ADDR = "127.0.0.1:0";
      env.VM_DATA_DIR = dir;
      const proc = spawn("vm", ["serve"], { env });
      proc.on("error", rej);
      let allbuf = new Uint8Array(0);
      const timer = setTimeout(() => rej("could not determine port"), 5000);
      proc.stderr.on("data", (data) => {
        const newA = new Uint8Array(allbuf.byteLength + data.byteLength);
        newA.set(allbuf);
        newA.set(data, allbuf.byteLength);
        allbuf = newA;
        const text = new TextDecoder().decode(allbuf);
        const find = text.match(/#voidmerged#listening:127.0.0.1:(\d+)#/);
        if (find && find.length >= 2) {
          clearTimeout(timer);
          res({ proc, port: parseInt(find[1]) });
        }
      });
    })) as { proc: ChildProcess; port: number };
    return new Vm(dir, proc, port);
  }

  port(): number {
    return this.#port;
  }

  async kill(): Promise<void> {
    this.#proc.kill("SIGKILL");
    await rm(this.#dir, {
      force: true,
      recursive: true,
      maxRetries: 3,
    });
  }
}
