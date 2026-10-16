import { core } from "ext:core/mod.js";

const webidl = core.loadExtScript("ext:deno_webidl/00_webidl.js");
Object.defineProperty(globalThis, webidl.brand, {
  value: webidl.brand,
  enumerable: false,
  configurable: true,
  writable: true,
});

const infra = core.loadExtScript("ext:deno_web/00_infra.js");
const url = core.loadExtScript("ext:deno_web/00_url.js");
const broadcastChannel = core.loadExtScript(
  "ext:deno_web/01_broadcast_channel.js",
);
const DOMException = core.loadExtScript("ext:deno_web/01_dom_exception.js");
const mimesniff = core.loadExtScript("ext:deno_web/01_mimesniff.js");
const urlPattern = core.loadExtScript("ext:deno_web/01_urlpattern.js");
const event = core.loadExtScript("ext:deno_web/02_event.js");
const structuredClone = core.loadExtScript(
  "ext:deno_web/02_structured_clone.js",
);
const timers = core.loadExtScript("ext:deno_web/02_timers.js");
const abortSignal = core.loadExtScript("ext:deno_web/03_abort_signal.js");
const globalInterfaces = core.loadExtScript(
  "ext:deno_web/04_global_interfaces.js",
);
const base64 = core.loadExtScript("ext:deno_web/05_base64.js");
const streams = core.loadExtScript("ext:deno_web/06_streams.js");
const encoding = core.loadExtScript("ext:deno_web/08_text_encoding.js");
const file = core.loadExtScript("ext:deno_web/09_file.js");
const fileReader = core.loadExtScript("ext:deno_web/10_filereader.js");
const location = core.loadExtScript("ext:deno_web/12_location.js");
const messagePort = core.loadExtScript("ext:deno_web/13_message_port.js");
const compression = core.loadExtScript("ext:deno_web/14_compression.js");
const performance = core.loadExtScript("ext:deno_web/15_performance.js");
const imageData = core.loadExtScript("ext:deno_web/16_image_data.js");
//const loadGeometry = core.createLazyLoader("ext:deno_web/geometry.js");
//const loadWebTransport = core.createLazyLoader("ext:deno_web/webtransport.js");
//const geometry = loadGeometry();
//const webTransport = loadWebTransport();

const WEB_LOAD = [
  ["AbortController", abortSignal.AbortController, false, true, true],
  ["AbortSignal", abortSignal.AbortSignal, false, true, true],
  ["Blob", file.Blob, false, true, true],
  ["BroadcastChannel", broadcastChannel.BroadcastChannel, false, true, true],
  //["ByteLengthQueuingStrategy, streams.ByteLengthQueuingStrategy, 		
  ["CloseEvent", event.CloseEvent, false, true, true],
  ["CompressionStream", compression.CompressionStream, false, true, true],
  //["CountQueuingStrategy, streams.CountQueuingStrategy, 		
  ["CustomEvent", event.CustomEvent, false, true, true],
  ["DecompressionStream", compression.DecompressionStream, false, true, true],
  ["DOMException", DOMException, false, true, true],
  ["ErrorEvent", event.ErrorEvent, false, true, true],
  ["Event", event.Event, false, true, true],
  ["EventTarget", event.EventTarget, false, true, true],
  ["File", file.File, false, true, true],
  ["FileReader", fileReader.FileReader, false, true, true],
  ["MessageEvent", event.MessageEvent, false, true, true],
  ["Performance", performance.Performance, false, true, true],
  ["PerformanceEntry", performance.PerformanceEntry, false, true, true],
  ["PerformanceMark", performance.PerformanceMark, false, true, true],
  ["PerformanceMeasure", performance.PerformanceMeasure, false, true, true],
  ["PromiseRejectionEvent", event.PromiseRejectionEvent, false, true, true],
  ["ProgressEvent", event.ProgressEvent, false, true, true],
  ["ReadableStream", streams.ReadableStream, false, true, true],
  //["ReadableStreamDefaultReader, streams.ReadableStreamDefaultReader, 		
  ["TextDecoder", encoding.TextDecoder, false, true, true],
  ["TextEncoder", encoding.TextEncoder, false, true, true],
  ["TextDecoderStream", encoding.TextDecoderStream, false, true, true],
  ["TextEncoderStream", encoding.TextEncoderStream, false, true, true],
  ["TransformStream", streams.TransformStream, false, true, true],
  ["URL", url.URL, false, true, true],
  ["URLPattern", urlPattern.URLPattern, false, true, true],
  ["URLSearchParams", url.URLSearchParams, false, true, true],
  ["MessageChannel", messagePort.MessageChannel, false, true, true],
  ["MessagePort", messagePort.MessagePort, false, true, true],
  ["WritableStream", streams.WritableStream, false, true, true],
  //["WritableStreamDefaultWriter, streams.WritableStreamDefaultWriter, 		
  //["WritableStreamDefaultController, streams.WritableStreamDefaultController, 		
  //["ReadableByteStreamController, streams.ReadableByteStreamController, 		
  //["ReadableStreamBYOBReader, streams.ReadableStreamBYOBReader, 		
  //["ReadableStreamBYOBRequest, streams.ReadableStreamBYOBRequest, 		
  //["ReadableStreamDefaultController, streams.ReadableStreamDefaultController, 		
  //["TransformStreamDefaultController, streams.TransformStreamDefaultController, 		
  ["ImageData", imageData.ImageData, false, true, true],
  ["atob", base64.atob, true, true, true],
  ["btoa", base64.btoa, true, true, true],
  ["clearInterval", timers.clearInterval, true, true, true],
  ["clearTimeout", timers.clearTimeout, true, true, true],
  ["performance", performance.performance, true, true, true],
  ["reportError", event.reportError, true, true, true],
  ["setInterval", timers.setInterval, true, true, true],
  ["setTimeout", timers.setTimeout, true, true, true],
  ["structuredClone", messagePort.structuredClone, true, true, true],
];

for (const [key, value, enumerable, configurable, writable] of WEB_LOAD) {
  Object.defineProperty(globalThis, key, {
    value,
    enumerable,
    configurable,
    writable,
  });
}

// Minimal console implementation. Each method forwards the lowercase method
// name as the "level" and the space-joined `toString()` of its arguments to
// the `op_log` op, which invokes the embedder-supplied log callback.
const consoleObj = { __proto__: null };
for (const level of ["debug", "log", "info", "warn", "error", "trace"]) {
  Object.defineProperty(consoleObj, level, {
    value: (...args) => {
      core.ops.op_log(level, args.map((arg) => arg.toString()).join(" "));
    },
    enumerable: true,
    configurable: true,
    writable: true,
  });
}
Object.defineProperty(globalThis, "console", {
  value: consoleObj,
  enumerable: false,
  configurable: true,
  writable: true,
});

const crypto = core.loadExtScript("ext:deno_crypto/00_crypto.js");

Object.defineProperty(globalThis, "CryptoKey", {
  value: crypto.CryptoKey,
  enumerable: false,
  configurable: true,
  writable: true,
});

Object.defineProperty(globalThis, "crypto", {
  value: crypto.crypto,
  enumerable: false,
  configurable: true,
  writable: false,
});

Object.defineProperty(globalThis, "Crypto", {
  value: crypto.Crypto,
  enumerable: false,
  configurable: true,
  writable: true,
});

Object.defineProperty(globalThis, "SubtleCrypto", {
  value: crypto.SubtleCrypto,
  enumerable: false,
  configurable: true,
  writable: true,
});
