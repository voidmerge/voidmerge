# voidmerge

[![NPM Client Version](https://img.shields.io/npm/v/%40voidmerge%2Fvoidmerge-client?label=voidmerge-client)](https://www.npmjs.com/package/@voidmerge/voidmerge-client)

[https://voidmerge.com](https://voidmerge.com)

[VoidMerge Handbook](https://voidmerge.com/book)

This is the web-client library for interacting with a VoidMerge server instance.

Please see the [Typescript API Docs](https://voidmerge.com/ts).

## Requirements

This library uses the native `fetch` API. It requires Node.js 18 or newer
(or any modern browser).

## Connection handling

Requests use native `fetch` (undici in Node) and include a per-request
timeout (default 30s) so a stalled connection fails instead of hanging
indefinitely. Each API call accepts optional `timeoutMs` and
`connectionClose` options:

- `timeoutMs` — per-request timeout in milliseconds.
- `connectionClose` — when `true`, sends `Connection: close` so a pooled
  connection is not reused. This is a safeguard for environments where
  stale keep-alive connections are suspected. Browsers ignore this header.

Node users can also tune undici's global connection pool, for example:

```js
import { Agent, setGlobalDispatcher } from "undici";

setGlobalDispatcher(
  new Agent({ keepAliveTimeout: 10_000, keepAliveMaxTimeout: 10_000 }),
);
```
