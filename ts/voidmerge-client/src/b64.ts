import * as b64 from "base64-js";

export function encode(a: Uint8Array): string {
  return b64
    .fromByteArray(a)
    .replace(/\//g, "_")
    .replace(/\+/g, "-")
    .replace(/\=/g, "");
}

export function decode(s: string): Uint8Array {
  let cur = s.replace(/\_/g, "/").replace(/\-/g, "+");
  while (cur.length % 4 !== 0) {
    cur = cur + "=";
  }
  return b64.toByteArray(cur);
}
