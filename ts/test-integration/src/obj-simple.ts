import * as VM from "@voidmerge/voidmerge-code";

VM.defineVoidMergeHandler(async (req) => {
  const reqType = req.type();

  if (req instanceof VM.RequestObjCheck) {
    return VM.RequestObjCheck.okResponse();
  } else if (req instanceof VM.RequestFn) {
    const pReq: {
      do: string;
      k: string;
      v: string;
    } = JSON.parse(new TextDecoder().decode(req.body()));

    const res: {
      meta?: string;
      list?: string[];
      val?: string;
    } = {};

    if (pReq.do === "put") {
      res.meta = (
        await VM.objPut(new TextEncoder().encode(pReq.v), {
          appPath: pReq.k,
          createdSecs: Date.now() / 1000.0,
        })
      ).fullPath();
    } else if (pReq.do === "list") {
      res.list = [];
      await VM.objList(pReq.k, async (metaList) => {
        for (const meta of metaList) {
          res.list?.push(meta.fullPath());
        }
      });
    } else if (pReq.do === "get") {
      res.val = new TextDecoder().decode(
        await VM.objGet(new VM.ObjMeta(pReq.k)),
      );
    }

    return VM.RequestFn.okResponse(
      200,
      new TextEncoder().encode(JSON.stringify(res)),
    );
  }

  throw new Error(`Invalid request type: ${reqType}`);
});
