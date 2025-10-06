import * as vm from "ext:core/ops";

globalThis.console = {
  log() {},
  error() {}
};

globalThis.TextEncoder = class TextEncoder {
  encode(s) {
    return vm.op_to_utf8(s);
  }
};

globalThis.TextDecoder = class TextDecoder {
  decode(b) {
    return vm.op_from_utf8(b);
  }
};

globalThis.objPut = vm.op_obj_put;
globalThis.objGet = vm.op_obj_get;
globalThis.objList = async function objList(pathPrefix, cb) {
  const ident = await vm.op_obj_list(pathPrefix);

  while (true) {
    const res = await vm.op_obj_list_check(ident);
    if (res) {
      await cb(res);
    } else {
      return;
    }
  }
}
