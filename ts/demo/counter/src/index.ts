import * as VM from "@voidmerge/voidmerge-code";

const META_PAGE = VM.ObjMeta.fromParts({ appPath: "page" });
const META_CRON = VM.ObjMeta.fromParts({ appPath: "cron" });

/**
 * Get current counter value.
 */
async function getCurrent(meta: VM.ObjMeta): Promise<number> {
  const { metaList } = await VM.objList({
    appPathPrefix: meta.appPath(),
    createdGt: 0,
    limit: 1,
  });

  if (metaList.length < 1) {
    // New install, return count 0
    return 0;
  }

  // get current value
  const { data } = await VM.objGet({ meta });

  // parse current value
  return parseInt(new TextDecoder().decode(data));
}

/**
 * Increment a counter.
 */
async function increment(meta: VM.ObjMeta): Promise<number> {
  // get the current value (or zero).
  // then add 1 to it.
  const count = (await getCurrent(meta)) + 1;

  // store the incremented value
  await VM.objPut({ meta, data: new TextEncoder().encode(count.toString()) });

  // return the incremented value
  return count;
}

// Configure cron interval.
VM.onCodeConfig(async (_req) => {
  return new VM.ResponseCodeConfigOk({
    // have our cron function run every 10 seconds.
    cronIntervalSecs: 10.0,
  });
});

// Execute cron code.
VM.onCron(async (_req) => {
  // when we get a cron request, increment the cron counter
  await increment(META_CRON);

  return new VM.ResponseCronOk();
});

// Object validation code.
VM.onObjCheck(async (req) => {
  const appPath = req.meta.appPath();

  // path can only be "cron" or "page".
  if (appPath !== "cron" && appPath !== "page") {
    throw new Error("invalid appPath");
  }

  // parse the data
  const num = parseInt(new TextDecoder().decode(req.data));

  // encoding len cannot be too long, and make sure it's a number
  if (req.data.byteLength > 32 || num < 1) {
    throw new Error("invalid data");
  }

  return new VM.ResponseObjCheckOk();
});

// Main api handler.
VM.onFn(async (req) => {
  try {
    // increment the page count
    const pageCount = await increment(META_PAGE);

    // fetch the cron count
    const cronCount = await getCurrent(META_CRON);

    // generate display text
    const output = `pageLoadCount: ${pageCount}\ncronTenSecondCount: ${cronCount}\n`;

    // return the response
    return new VM.ResponseFnOk().text(output);
  } catch (e: any) {
    try {
      return new VM.ResponseFnOk()
        .withStatus(500)
        .text(`Error: ${e.toString()}`);
    } catch (_: any) {
      throw e;
    }
  }
});
