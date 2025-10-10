import * as VM from "@voidmerge/voidmerge-code";

VM.defineVoidMergeHandler(async (req) => {
  const reqType = req.type();

  if (req instanceof VM.RequestObjCheck) {
    return VM.RequestObjCheck.okResponse();
  } else if (req instanceof VM.RequestFn) {
    return VM.RequestFn.okResponse(
      200,
      new TextEncoder().encode(JSON.stringify(req)),
    );
  }

  throw new Error(`Invalid request type: ${reqType}`);
});
