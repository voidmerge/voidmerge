# Functions

Functions represent the main request/response type of a Void Merge service.

[RequestFn type API Docs](https://www.voidmerge.com/ts/classes/VoidMergeCode.RequestFn)

```ts
// Import the voidmerge-code library to gain access to types.
import * as VM from "@voidmerge/voidmerge-code"

// Main Void Merge API Handler
VM.onFn(async (req) => {
  return new VM.ResponseFnOk().text("Hello World");
})
```
