# Functions

Functions represent the main request/response type of a Void Merge service.

[RequestFn type API Docs](https://www.voidmerge.com/ts/classes/VoidMergeCode.RequestFn)

```ts
// Import the voidmerge-code library to gain access to types.
import * as VM from "@voidmerge/voidmerge-code"

// The "VoidMergeHandler" is the main entry point to void merge logic.
VM.defineVoidMergeHandler(async (req) => {
  // If this is a "Function" type request, send back a "Hello World!" response.
  if (req instanceof VM.RequestFn) {
    return new VM.ResponseFnOk({
      // Set the http status code to 200.
      status: 200,

      // Send back "Hello World!" text.
      body: new TextEncoder().encode("Hello World!"),

      // Set the content type to "text/plain".
      headers: {
        "content-type": "text/plain",
      },
    });
  }

  // Output an error if the request is a type we don't handle.
  throw new Error(`Unhandled Request ${JSON.stringify(req)}`);
})
```
