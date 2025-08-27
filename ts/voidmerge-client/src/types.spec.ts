import * as types from "./types.js";

describe("types", () => {
  it("VmHash.toJSON", () => {
    const h = types.VmHash.parse("AAAA");
    const obj = { h };
    const repr = JSON.stringify(obj);
    expect(repr).toEqual('{"h":"AAAA"}');
  });
});
