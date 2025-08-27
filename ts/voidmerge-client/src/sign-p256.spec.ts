import { VmSignP256 } from "./sign-p256.js";

describe("sign-p256", () => {
  it("sanity", () => {
    const mod = new VmSignP256();

    const sec = mod.genSecret();
    const pub = mod.genPublic(sec);

    const msg = new Uint8Array([42, 39, 200]);

    const sig = mod.sign(sec, msg);

    expect(mod.verify(sig, pub, msg)).toEqual(true);

    msg[2] = 201;

    expect(mod.verify(sig, pub, msg)).toEqual(false);
  });
});
