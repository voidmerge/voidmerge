# Object Store Validation

When data is put into the object store, either from a call in a function or
cron handler, or when it is synced from a peer node in a cluster, Void Merge
provides an opportunity to validate the data, and reject it if needed.

[RequestObjCheck type API Docs](https://www.voidmerge.com/ts/classes/VoidMergeCode.RequestObjCheck)

```ts
import * as VM from "@voidmerge/voidmerge-code";

VM.defineVoidMergeHandler(async (req) => {
  if (req instanceof VM.RequestObjCheck) {
    const data = new TextDecoder().decode(req.data);
    if (data === "hello") {
      return new VM.ResponseObjCheckOk();
    } else {
      throw new Error(`invalid data: ${data}`);
    }
  }
}
```
