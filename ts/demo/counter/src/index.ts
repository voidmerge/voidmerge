import * as VM from "@voidmerge/voidmerge-code";

const META_PAGE = VM.ObjMeta.fromParts({ appPath: "page" });
const META_CRON = VM.ObjMeta.fromParts({ appPath: "cron" });

/**
 * Get current counter value.
 */
async function getCurrent(meta: VM.ObjMeta): Promise<number> {
  let count = 0;
  try {
    // get current value
    const { data } = await VM.objGet({ meta });

    // parse current value
    count = parseInt(new TextDecoder().decode(data));
  } catch (_e: any) {
    /* pass */
  }
  return count;
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

/**
 * Main dispatch handler.
 */
VM.defineVoidMergeHandler(async (req) => {
  const type = req.type;

  if (req instanceof VM.RequestCodeConfig) {
    return new VM.ResponseCodeConfigOk({
      // have our cron function run every 10 seconds.
      cronIntervalSecs: 10.0,
    });
  } else if (req instanceof VM.RequestCron) {
    // when we get a cron request, increment the cron counter
    await increment(META_CRON);

    return new VM.ResponseCronOk();
  } else if (req instanceof VM.RequestObjCheck) {
    // validate data to store in the object store

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
  } else if (req instanceof VM.RequestFn) {
    // increment the page count
    const pageCount = await increment(META_PAGE);

    // fetch the cron count
    const cronCount = await getCurrent(META_CRON);

    // generate display text
    const output = `pageLoadCount: ${pageCount}\ncronTenSecondCount: ${cronCount}\n`;

    // return the response
    return new VM.ResponseFnOk({
      status: 200,
      body: new TextEncoder().encode(output),
      headers: {
        "content-type": "text/plain",
      },
    });
  } else {
    throw new Error(`invalid request type: ${type}`);
  }
});
