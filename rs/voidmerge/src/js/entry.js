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

let ctx = null;
globalThis.VM = {
  ctx: () => {
    if (ctx) {
      return ctx;
    }
    ctx = vm.op_get_ctx();
    return ctx;
  },
  msgNew: vm.op_msg_new,
  msgList: vm.op_msg_list,
  msgSend: vm.op_msg_send,
  objPut: vm.op_obj_put,
  objGet: vm.op_obj_get,
  objList: vm.op_obj_list
};
