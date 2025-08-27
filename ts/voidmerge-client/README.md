# voidmerge

![Crates.io License](https://img.shields.io/crates/l/voidmerge)
![Crates.io Version](https://img.shields.io/crates/v/voidmerge)
![NPM Version](https://img.shields.io/npm/v/%40voidmerge%2Fvoidmerge-client)

[https://voidmerge.com](https://voidmerge.com)

[VoidMerge Handbook](https://voidmerge.com/book)

This is the web-client library for interacting with a VoidMerge server instance.

Please see the [Typescript API Docs](https://voidmerge.com/ts).

```ts
// load the client library
import * as VM from "@voidmerge/voidmerge-client";

// construct a signer instance
const sign = new VM.VmMultiSign();

// load up the P256 algorithm
sign.addSign(new VM.VmSignP256());

// TODO load the peristance with `sign.loadEncoded(..)`,
//      OR persist this somewhere with `sign.encode()`.

// finally, create the actual client
const vm = new VM.VoidMergeClient(
  sign,
  new URL("http://127.0.0.1:8080"),
  VM.VmHash.parse("AAAA"),
);
```
