import * as VM from "@voidmerge/voidmerge-code";

VM.defineVoidMergeHandler(async (req) => {
  const reqType = req.type;

  if (req instanceof VM.RequestCodeConfig) {
    // run cron every 10 ms
    return new VM.ResponseCodeConfigOk({ cronIntervalSecs: 0.01 });
  } else if (req instanceof VM.RequestObjCheck) {
    // approve everything
    return new VM.ResponseObjCheckOk();
  } else if (req instanceof VM.RequestCron) {
    // write a new unique entry every cron run
    await VM.objPut({
      meta: VM.ObjMeta.fromParts({ appPath: Date.now().toString() }),
      data: new TextEncoder().encode(Date.now().toString()),
    });

    return new VM.ResponseCronOk();
  } else if (req instanceof VM.RequestFn) {
    // print out a count of all entries
    const { metaList } = await VM.objList({
      appPathPrefix: "",
      createdGt: 0.0,
      limit: 1000,
    });
    return new VM.ResponseFnOk({
      status: 200,
      body: new TextEncoder().encode(JSON.stringify(metaList.length)),
    });
  }

  throw new Error(`Invalid request type: ${reqType}`);
});
