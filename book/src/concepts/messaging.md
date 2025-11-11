# Messaging

A Void Merge service provides the ability to send messages to connected clients
and can be set up so clients can message each other.

## Create a new messaging channel for a client.

[msgNew API Docs](https://www.voidmerge.com/ts/functions/VoidMergeCode.msgNew)

```ts
const { msgId } = await VM.msgNew();
```

[MsgListener API Docs](https://www.voidmerge.com/ts/functions/VoidMergeCode.msgNew)

```ts
// In the client code:

import * as VM from "@voidmerge/voidmerge-client"

const listener = await VM.MsgListener.connect({
    msgId,
    ...
});
```

## List open messaging channels.

[msgList API Docs](https://www.voidmerge.com/ts/functions/VoidMergeCode.msgList)

```ts
const { msgIdList } = await VM.msgList();
```

## Send a message over an open channel.

[msgSend API Docs](https://www.voidmerge.com/ts/functions/VoidMergeCode.msgSend)

```ts
await VM.msgSend({
    msg: new TextEncoder().encode("hello"),
    msgId,
});
```
