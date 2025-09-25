import * as vm from "ext:core/ops";

/*
globalThis.console = {
  log() {},
  error() {}
};
*/

globalThis.TextEncoder = class TextEncoder {
  encode(s) {
    return vm.op_vm_to_utf8(s);
  }
};

globalThis.TextDecoder = class TextDecoder {
  decode(b) {
    return vm.op_vm_from_utf8(b);
  }
};

async function sleep(ms) {
  return new Promise((res) => {
    const id = setTimeout(() => {
      clearTimeout(id);
      res()
    }, ms);
  });
}

globalThis.objPut = vm.op_obj_put;
globalThis.objGet = vm.op_obj_get;
globalThis.objList = async function objList(pathPrefix, cb) {
  const ident = vm.op_obj_list(pathPrefix);
  for (let i = 0; i < 588; ++i) {
    const [code, res] = vm.op_obj_list_check(ident);
    if (Array.isArray(res) && res.length) {
      cb(res);
    }
    if (code === 0) {
      return;
    }
    await sleep(17);
  }
}
