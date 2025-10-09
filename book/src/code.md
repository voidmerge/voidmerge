# Code

VoidMerge logic is written in typescript, using the `@voidmerge/voidmerge-code`
package.

```ts
import * as VM from "@voidmerge/voidmerge-code";
```

## Define a handler

VoidMerge handles execution through defining an async handler function.

```ts
VM.defineVoidMergeHandler(async (req) => {
    ...
});
```

## Handle an incoming function request

When a user navigates to a path in your context, your hander function will be
invoked.

```ts
VM.defineVoidMergeHandler(async (req) => {
    if (req instanceof VM.RequestFn) {
        return VM.RequestFn.okResponse(
            200,
            new TextEncoder().encode("hello"),
        );
    }
});
```

## Use the object store

Your VoidMerge code has access to an object store associated with your context.

```ts
await VM.objPut({
    meta: VM.ObjMeta.fromParts({ appPath: "test" }),
    data: new TextEncoder().encode("hello"),
});

await VM.objGet({
    meta: VM.ObjMeta.fromParts({ appPath: "test" }),
});
```

## Handle object validation

When an object is inserted into the object store, you have a chance to validate
it before it is actually accepted.

```ts
VM.defineVoidMergeHandler(async (req) => {
    if (req instanceof VM.RequestObjCheck) {
        const data = new TextDecoder().decode(req.data());

        if (data !== "hello") {
            return new Error("Oh, no!");
        }

        // Everything looks good!
        return VM.RequestObjCheck.okResponse();
    }
});
```
