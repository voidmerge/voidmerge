import * as VM from "@voidmerge/voidmerge-code";

VM.defineVoidMergeHandler(async (req) => {
  const reqType = req.type;

  if (req instanceof VM.RequestObjCheck) {
    const data = new TextDecoder().decode(req.data);
    if (data === "hello") {
      return new VM.ResponseObjCheckOk();
    } else {
      throw new Error(`invalid data: ${data}`);
    }
  } else if (req instanceof VM.RequestFn) {
    const pReq: {
      do: string;
      k: string;
      v: string;
    } = JSON.parse(new TextDecoder().decode(req.body));

    const res: {
      meta?: string;
      list?: string[];
      val?: string;
    } = {};

    if (pReq.do === "put") {
      res.meta = (
        await VM.objPut({
          meta: VM.ObjMeta.fromParts({ appPath: pReq.k }),
          data: new TextEncoder().encode(pReq.v),
        })
      ).meta.fullPath();
    } else if (pReq.do === "list") {
      res.list = [];
      const { metaList } = await VM.objList({
        appPathPrefix: pReq.k,
        createdGt: 0.0,
        limit: 42,
      });
      for (const meta of metaList) {
        res.list?.push(meta.fullPath());
      }
    } else if (pReq.do === "get") {
      const { data } = await VM.objGet({
        meta: VM.ObjMeta.fromFull(pReq.k),
      });
      res.val = new TextDecoder().decode(data);
    }

    return new VM.ResponseFnOk({
      status: 200,
      body: new TextEncoder().encode(JSON.stringify(res)),
    });
  }

  throw new Error(`Invalid request type: ${reqType}`);
});
