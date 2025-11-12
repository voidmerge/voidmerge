# Cron

Void Merge web services provide a method for periodic maintenance tasks.

First, we have to tell void merge how often to run this task.

[ResponseCodeConfigOk type API Docs](https://www.voidmerge.com/ts/classes/VoidMergeCode.ResponseCodeConfigOk)

```ts
import * as VM from "@voidmerge/voidmerge-code";

VM.defineVoidMergeHandler(async (req) => {
  const reqType = req.type;

  if (req instanceof VM.RequestCodeConfig) {
    // Run cron every 10 seconds.
    return new VM.ResponseCodeConfigOk({ cronIntervalSecs: 10 });
  }
}
```

Now we need to actually do something when a cron request is triggered.

```ts
import * as VM from "@voidmerge/voidmerge-code";

VM.defineVoidMergeHandler(async (req) => {
  const reqType = req.type;

  if (req instanceof VM.RequestCodeConfig) {
    // Run cron every 10 seconds.
    return new VM.ResponseCodeConfigOk({ cronIntervalSecs: 10 });
  } else if (req instanceof VM.RequestCron) {
    // Write a new unique entry every cron run.
    await VM.objPut({
      meta: VM.ObjMeta.fromParts({ appPath: Date.now().toString() }),
      data: new TextEncoder().encode(Date.now().toString()),
    });

    return new VM.ResponseCronOk();
  }
}
```
