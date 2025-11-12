# Object Store

A Void Merge service has access to an object store.

## Put data into the store.

[objPut API Docs](https://www.voidmerge.com/ts/functions/VoidMergeCode.objPut)

```ts
const { meta } = await VM.objPut({
    meta: VM.ObjMeta.fromParts({ appPath: "test-path" }),
    data: new TextEncoder().encode("test-data"),
});
```

## Get data from the store.

[objGet API Docs](https://www.voidmerge.com/ts/functions/VoidMergeCode.objGet)

```ts
const { data } = await VM.objGet({
    meta: VM.ObjMeta.fromParts({ appPath: "test-path" }),
});
```

## List data from the store.

[objList API Docs](https://www.voidmerge.com/ts/functions/VoidMergeCode.objList)

```ts
const { metaList } = await VM.objList({
    appPathPrefix: "test-",
    createdGt: 0,
    limit: 50,
});
```
