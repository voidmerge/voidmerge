import * as VM from "@voidmerge/voidmerge-code";
import assets from "./assets.js";
import { b64Dec } from "./b64.js";
import { avatarSvg } from "./avatar.js";

/**
 * Main dispatch handler.
 */
VM.defineVoidMergeHandler(async (req) => {
  const type = req.type;
  if (req instanceof VM.RequestCodeConfig) {
    return new VM.ResponseCodeConfigOk({
      cronIntervalSecs: 60.0 * 10.0, // 10 minutes
    });
  } else if (req instanceof VM.RequestCron) {
    return await handleCron();
  } else if (req instanceof VM.RequestObjCheck) {
    return await handleObjCheck(req);
  } else if (req instanceof VM.RequestFn) {
    return await handleFn(req);
  } else {
    throw new Error(`invalid request type: ${type}`);
  }
});

/**
 * Handle our periodic maintenance tasks.
 */
async function handleCron(): Promise<VM.ResponseCronOk> {
  return new VM.ResponseCronOk();
}

/**
 * Validate data to be added to the object store.
 */
async function handleObjCheck(
  req: VM.RequestObjCheck,
): Promise<VM.ResponseObjCheckOk> {
  return new VM.ResponseObjCheckOk();
}

/**
 * Handle user requests.
 */
async function handleFn(req: VM.RequestFn): Promise<VM.ResponseFnOk> {
  if (req.path === "cron") {
    await handleCron();
    return new VM.ResponseFnOk({
      status: 200,
      body: new TextEncoder().encode("Ok"),
      headers: {
        "content-type": "text/plain",
      },
    });
  } else if (req.path === "env") {
    return new VM.ResponseFnOk({
      status: 200,
      body: new TextEncoder().encode(JSON.stringify(VM.env())),
      headers: {
        "content-type": "application/json",
      },
    });
  } else if (req.path.startsWith("league/")) {
    let leagueNum = parseInt(req.path.split("/")[1] || "0") || 0;
    if (leagueNum < 1) {
      leagueNum = 1;
    }
    if (leagueNum > 9) {
      leagueNum = 9;
    }
    return new VM.ResponseFnOk({
      status: 200,
      body: new TextEncoder().encode(
        JSON.stringify({
          leagueNum,
        }),
      ),
      headers: {
        "content-type": "application/json",
      },
    });
  } else if (req.path === "publish") {
    const raw = JSON.parse(new TextDecoder().decode(req.body));
    if (!Array.isArray(raw)) {
      throw new Error("publish body must be an array");
    }
    const path = raw.shift();
    if (typeof path !== "string" || !path.startsWith("stars~")) {
      throw new Error("invalid path");
    }

    const data = new TextEncoder().encode(JSON.stringify(raw));

    const { meta } = await VM.objPut({
      meta: VM.ObjMeta.fromParts({
        appPath: path,
        // 1 week
        expiresSecs: Date.now() / 1000 + 60 * 60 * 24 * 7,
      }),
      data,
    });

    return new VM.ResponseFnOk({
      status: 200,
      body: new TextEncoder().encode(meta.fullPath()),
      headers: {
        "content-type": "application/json",
      },
    });
  } else if (req.path === "favicon.svg") {
    return new VM.ResponseFnOk({
      status: 200,
      body: new TextEncoder().encode(assets["favicon.svg"]),
      headers: {
        "content-type": "image/svg+xml",
      },
    });
  } else if (req.path === "index.css") {
    return new VM.ResponseFnOk({
      status: 200,
      body: new TextEncoder().encode(assets["index.css"]),
      headers: {
        "content-type": "text/css",
      },
    });
  } else if (req.path === "index.js") {
    return new VM.ResponseFnOk({
      status: 200,
      body: new TextEncoder().encode(assets["index.js"]),
      headers: {
        "content-type": "application/javascript",
      },
    });
  } else if (req.path.startsWith("avatar/")) {
    const avatarCode = b64Dec(req.path.split("/")[1]);
    if (avatarCode.byteLength !== 16) {
      throw new Error("invalid avatar code");
    }
    const svg = avatarSvg(avatarCode);
    return new VM.ResponseFnOk({
      status: 200,
      body: new TextEncoder().encode(svg),
      headers: {
        "content-type": "image/svg+xml",
      },
    });
  }

  return new VM.ResponseFnOk({
    status: 200,
    body: new TextEncoder().encode(assets["index.html"]),
    headers: {
      "content-type": "text/html",
    },
  });
}
