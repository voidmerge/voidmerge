# Env

In VoidMerge, "Env" is global metadata associated with a VoidMerge context.

## Example VoidMerge Env

```json
{
  "public": {
    "servers": ["http://127.0.0.1:8080"],
    "examplePublicEnv": "hello"
  },
  "private": {
    "ctxadminPubkeys": [],
    "examplePrivateEnv": "world"
  }
}
```

## Public

The "public" data will be available on the "status" call of the VM server which is not authenticated. It helps clients know how to connect / bootstrap into the network. Do not share any privileged information in this section.

## Private

The "private" data will only be available to clients that have authenticated.

## In logic

In the logic code, the env data is available on the input parameter object as the property `env`.

```javascript
VM({
  call: "register",
  code(i) {
    if (
      i.env.public.examplePublicEnv !== "hello"
      || i.env.private.examplePrivateEnv !== "world"
    ) {
      throw new Error("Invalid Env Data");
    }

    return { result: "valid" };
  }
});
```

## Meta Storage and Validation

Logic is stored in a VoidMerge context the same as any other data. It is stored with the "sysenv" type.
