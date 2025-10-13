import * as VM from "@voidmerge/voidmerge-code";

VM.defineVoidMergeHandler(async (req) => {
  const reqType = req.type;

  if (req instanceof VM.RequestObjCheck) {
    return new VM.ResponseObjCheckOk();
  } else if (req instanceof VM.RequestFn) {
    const path = req.path;
    let out = `use listen or sendall (got: '${path}')`;
    if (path === "listen") {
      const { msgId } = await VM.msgNew();
      out = msgId + "\n";
    } else if (path === "sendall") {
      const msg = req.body;
      const msgStr = new TextDecoder().decode(msg);
      const { msgIdList } = await VM.msgList();
      for (const msgId of msgIdList) {
        await VM.msgSend({ msg, msgId });
      }
      out = `sent: ${msgStr}\n`;
    }

    return new VM.ResponseFnOk(200, new TextEncoder().encode(out));
  }

  throw new Error(`Invalid request type: ${reqType}`);
});
