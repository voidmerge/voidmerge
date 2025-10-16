import * as VM from "@voidmerge/voidmerge-code";
import assets from "./assets.js";
import { b64Dec } from "./b64.js";
import { avatarSvg } from "./avatar.js";

VM.defineVoidMergeHandler(async (req) => {
  const type = req.type;
  if (req instanceof VM.RequestObjCheck) {
    return await handleObjCheck(req);
  } else if (req instanceof VM.RequestFn) {
    return await handleFn(req);
  } else {
    throw new Error(`invalid request type: ${type}`);
  }
});

async function handleObjCheck(
  req: VM.RequestObjCheck,
): Promise<VM.ResponseObjCheckOk> {
  return new VM.ResponseObjCheckOk();
}

async function handleFn(req: VM.RequestFn): Promise<VM.ResponseFnOk> {
  if (req.path === "favicon.svg") {
    return new VM.ResponseFnOk(
      200,
      new TextEncoder().encode(assets["favicon.svg"]),
      {
        "content-type": "image/svg+xml",
      },
    );
  } else if (req.path === "index.css") {
    return new VM.ResponseFnOk(
      200,
      new TextEncoder().encode(assets["index.css"]),
      {
        "content-type": "text/css",
      },
    );
  } else if (req.path === "index.js") {
    return new VM.ResponseFnOk(
      200,
      new TextEncoder().encode(assets["index.js"]),
      {
        "content-type": "application/javascript",
      },
    );
  } else if (req.path.startsWith("avatar/")) {
    const avatarCode = b64Dec(req.path.split("/")[1]);
    if (avatarCode.byteLength !== 8) {
      throw new Error("invalid avatar code");
    }
    const svg = avatarSvg(avatarCode);
    return new VM.ResponseFnOk(200, new TextEncoder().encode(svg), {
      "content-type": "image/svg+xml",
    });
  }

  return new VM.ResponseFnOk(
    200,
    new TextEncoder().encode(assets["index.html"]),
    {
      "content-type": "text/html",
    },
  );
}
