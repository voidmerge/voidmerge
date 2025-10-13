import { ObjMeta } from "./obj-meta.js";
import { checkTestSetup } from "./test-setup.js";

describe("ObjMeta", () => {
  beforeEach(checkTestSetup);

  it("fromParts", () => {
    const meta = ObjMeta.fromParts({ appPath: "test" });
    expect(meta.appPath()).toEqual("test");
  });
});
