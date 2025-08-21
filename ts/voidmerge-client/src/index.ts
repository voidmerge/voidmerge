/**
 * Typescript client for interacting with a VoidMerge server.
 *
 * @see {@link VoidMergeClient} - The main entry point
 * for working with VoidMerge.
 *
 * @example
 * ```ts
 * // load the client library
 * import * as VM from "@voidmerge/voidmerge-client";
 *
 * // construct a signer instance
 * const sign = new VM.VmMultiSign();
 *
 * // load up the P256 algorithm
 * sign.addSign(new VM.VmSignP256());
 *
 * // TODO persist this somewhere with `sign.encode()`.
 * // TODO load the peristance with `sign.loadEncoded(..)`.
 *
 * // finally, create the actual client
 * const vm = new VM.VoidMergeClient(
 *   sign,
 *   new URL("http://127.0.0.1:8080"),
 *   VM.VmHash.parse("AAAA"),
 * );
 * ```
 *
 * @packageDocumentation
 */

export * from "./types";
export * from "./sign-p256";
export * from "./http-client";
export * from "./void-merge-client";
