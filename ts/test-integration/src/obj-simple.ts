import * as VM from "@voidmerge/voidmerge-code";

VM.defineVoidMergeHandler(async (req) => {
  const reqType = req.type();

  if (req instanceof VM.RequestObjCheck) {
    const data = new TextDecoder().decode(req.data());
    if (data === "hello") {
      return VM.RequestObjCheck.okResponse();
    } else {
      throw new Error(`invalid data: ${data}`);
    }
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
        })
      ).fullPath();
    } else if (pReq.do === "list") {
      res.list = [];
      for (const meta of await VM.objList(pReq.k, 0.0, 42)) {
        res.list?.push(meta.fullPath());
      }
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
