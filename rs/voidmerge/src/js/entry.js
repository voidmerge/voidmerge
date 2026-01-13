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

function frz(obj) {
  Object.freeze(obj);

  for (const k in obj) {
    const v = obj[k];
    if (v && typeof v === "object") {
      frz(v);
    }
  }

  return obj;
}

let cache = null;
function getCache() {
  if (!cache) {
    cache = frz({
      ctx: vm.op_get_ctx(),
      env: vm.op_get_env(),
    });
  }
  return cache;
}

globalThis.VM = {
  ctx: () => { return getCache().ctx; },
  env: () => { return getCache().env; },
  msgNew: vm.op_msg_new,
  msgList: vm.op_msg_list,
  msgSend: vm.op_msg_send,
  objPut: vm.op_obj_put,
  objGet: vm.op_obj_get,
  objRm: vm.op_obj_rm,
  objList: vm.op_obj_list
};
