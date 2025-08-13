VM({
  call: "register",
  code(i) {
    if (i.call === "validate") {
      return validate(i);
    }
    return "unimplemented";
  },
});

function validate(i) {
  if (i.type === "sysenv" || i.type === "syslogic" || i.type === "sysweb") {
    // approve all the system types.
  } else if (i.type === "unique") {
    // the "unique" type tracks how many people have ever connected
    // to the server.
    const parsed = VM({ call: "system", type: "vmDecode", data: i.data.enc });
    if (parsed.app) {
      throw new TypeError("type 'unique' cannot have app data");
    }
  } else if (i.type === "online") {
    // the "online" type tracks who is currently online.
    const parsed = VM({ call: "system", type: "vmDecode", data: i.data.enc });
    if (typeof parsed.app !== "string") {
      throw new TypeError("type 'online' app data must be a string");
    }
    if (parsed.app.length > 128) {
      throw new Error(
        "online status message is too long (must be < 128 characters",
      );
    }
  } else {
    // don't allow any other types.
    throw new Error(`invalid type: ${i.type}`);
  }

  // if we did not throw above, this item must be valid.
  return { result: "valid" };
}
