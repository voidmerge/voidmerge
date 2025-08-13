import * as types from "./types";

describe("types", () => {
  it("VmHash.toJSON", () => {
    const h = types.VmHash.parse("AAAA");
    const obj = { h };
    const repr = JSON.stringify(obj);
    expect(repr).toEqual('{"h":"AAAA"}');
  });
});
