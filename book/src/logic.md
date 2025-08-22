# Logic

In VoidMerge, "Logic" is the code that defines how data is handled, and whether it is valid and will be accepted, or invalid and will be rejected.

## Simple Starter Logic

The simplest logic definition allows all data, and looks like this:

```javascript
VM({
  call: "register",
  code(_i) {
    return { result: "valid" };
  }
});
```

Let's break this down. First, the global `VM` function:

## The `VM` global function

```javascript
VM(
  /* .. */
);
```

This is the main entrypoint provided in the execution context of the VoidMerge logic. It can do multiple things, including providing access to system APIs, but what we care about at the moment is registering the validation logic for VoidMerge structured data.

## Call type "register"

```javascript
VM({
  call: "register",
  /* .. */
});
```

We always pass an object to the `VM` function. By setting the "call" property to "register", we are telling VoidMerge that we are registering validation logic.

## Finally, the code function itself

```javascript
VM({
  /* .. */
  code(_i) {
    return { result: "valid" };
  }
});
```

The "code" function takes an input parameter, which we are ignoring here, and returns a validation result. In this case, we are returning that the data is valid, thus it will be allowed / stored.

## Meta Storage and Validation

Logic is stored in a VoidMerge context the same as any other data. It is stored with the "syslogic" type.
