import * as VM from "@voidmerge/voidmerge-client";

const CTX: VM.VmHash = VM.VmHash.parse("AAAA");

function getElem(id: string): HTMLElement {
  const elem = document.getElementById(id);
  if (!elem) {
    throw new Error(`could not find '${id}' element`);
  }
  return elem;
}

class Example1 {
  #unique: VM.VmHash;
  #vm: VM.VoidMergeClient;
  #disp: HTMLElement;
  #elemUniqueEverCount: HTMLElement;
  #uniqueEverCount: number;
  #elemOnlineCount: HTMLElement;
  #onlineCount: number;
  #elemStatus: HTMLInputElement;
  #elemShout: HTMLInputElement;
  #disable: Array<HTMLInputElement>;

  constructor(url: URL) {
    this.#disp = getElem("display");
    this.#disable = [];

    const sign = new VM.VmMultiSign();
    sign.addSign(new VM.VmSignP256());

    let persist = globalThis.localStorage.getItem("multiSign");

    if (!persist) {
      persist = sign.encode();
      globalThis.localStorage.setItem("multiSign", persist);
    }

    let unique = globalThis.localStorage.getItem("unique");

    if (!unique) {
      unique = VM.VmHash.nonce().toString();
      globalThis.localStorage.setItem("unique", unique);
    }

    this.#unique = VM.VmHash.parse(unique);

    this.print(`unique: ${this.#unique.toString()}`);

    sign.loadEncoded(persist);

    this.#vm = new VM.VoidMergeClient(sign, url, CTX);

    this.#vm.setShortCache(new VM.VmObjSignedShortCacheLru(1024 * 16));
    this.#vm.setApiToken(VM.VmHash.parse("bobo"));

    this.#elemUniqueEverCount = getElem("uniqueEverCount");
    this.#uniqueEverCount = 0;
    this.#elemOnlineCount = getElem("onlineCount");
    this.#onlineCount = 0;

    this.#elemStatus = getElem("status") as HTMLInputElement;
    this.#disable.push(this.#elemStatus);
    this.#disable.push(getElem("setStatus") as HTMLInputElement);
    this.#elemShout = getElem("shout") as HTMLInputElement;
    this.#disable.push(this.#elemShout);
    this.#disable.push(getElem("sendShout") as HTMLInputElement);

    getElem("statusForm").onsubmit = (evt) => {
      evt.preventDefault();
      evt.stopPropagation();
      this.putOnline();
    };

    getElem("shoutForm").onsubmit = (evt) => {
      evt.preventDefault();
      evt.stopPropagation();
      const msg = this.#elemShout.value;
      this.#elemShout.value = "";
      this.shout(msg);
    };

    this.#vm.setMessageCallback((msg) => {
      this.print(new TextDecoder().decode(msg.data));
    });

    this.disable();
  }

  setUniqueEverCount(v: number) {
    this.#uniqueEverCount = v;
    this.#elemUniqueEverCount.innerHTML = this.#uniqueEverCount.toString();
  }

  setOnlineCount(v: number) {
    this.#onlineCount = v;
    this.#elemOnlineCount.innerHTML = this.#onlineCount.toString();
  }

  disable() {
    for (const elem of this.#disable) {
      elem.disabled = true;
    }
  }

  enable() {
    for (const elem of this.#disable) {
      elem.disabled = false;
    }
  }

  print(txt: string) {
    for (const line of txt.split("\n")) {
      const tline = line.trim();
      if (tline.length > 0) {
        this.#disp.insertBefore(
          document.createTextNode(tline + "\n"),
          this.#disp.childNodes[0],
        );
      }
    }
  }

  async init(): Promise<void> {
    this.print("Loading...");

    await this.#vm.insert(
      new VM.VmObj("unique")
        // keep these for 3 days
        .withTtlS(Date.now() / 1000 + 60 * 60 * 24 * 3)
        .withIdent(this.#unique),
    );

    await this.checkPeers();
    setInterval(() => {
      this.checkPeers();
    }, 10000);

    this.print("Ready.");

    this.enable();
  }

  async checkPeers(): Promise<void> {
    await this.putOnline();

    const ever = await this.#vm.select(
      new VM.VmSelect().withFilterByTypes(["unique"]),
    );
    this.setUniqueEverCount(ever.count);

    const online = await this.#vm.select(
      new VM.VmSelect().withFilterByTypes(["online"]).withReturnData(true),
    );
    this.setOnlineCount(online.count);

    const peers: Set<string> = new Set();
    if (Array.isArray(online.results)) {
      for (const peer of online.results) {
        if (peer.data && typeof peer.data.parsed.app === "string") {
          peers.add(peer.data.parsed.app);
        }
      }
    }

    this.setPeers(peers);
  }

  setPeers(peers: Set<string>) {
    const remove = [];
    const found = new Set();
    const online = getElem("online");
    for (let i = 0; i < online.childNodes.length; ++i) {
      const child = online.childNodes[i] as HTMLElement;
      found.add(child.innerHTML);
      if (!peers.has(child.innerHTML)) {
        remove.push(child);
      }
    }
    for (const child of remove) {
      online.removeChild(child);
    }
    for (const peer of peers) {
      if (!found.has(peer)) {
        const child = document.createElement("span");
        child.className = "online";
        child.innerHTML = peer;
        online.appendChild(child);
      }
    }
  }

  async shout(msg: string): Promise<void> {
    const enc = new TextEncoder().encode(this.#elemStatus.value + ": " + msg);
    const online = await this.#vm.select(
      new VM.VmSelect().withFilterByTypes(["online"]).withReturnIdent(true),
    );
    if (Array.isArray(online.results)) {
      for (const peer of online.results) {
        if (peer.ident) {
          try {
            await this.#vm.send(peer.ident, enc);
          } catch (_e) {
            /* pass */
          }
        }
      }
    }
  }

  async putOnline(): Promise<void> {
    const thisPeerHash = await this.#vm.getThisPeerHash();
    await this.#vm.insert(
      new VM.VmObj("online")
        .withTtlS(Date.now() / 1000 + 30)
        .withIdent(thisPeerHash)
        .withApp(this.#elemStatus.value),
    );
  }
}

async function main() {
  const url = new URL(document.location.href);
  url.pathname = "/";

  const ex = new Example1(url);

  await ex.init();
}

main().then(
  () => {},
  (err) => {
    console.error(err);
  },
);
