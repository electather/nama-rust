import { Code, ConnectError } from "@connectrpc/connect";

/** Whether `error`, from an RPC or not, carries the Connect `code`. */
export function isCode(error: unknown, code: Code) {
  return ConnectError.from(error).code === code;
}

// Retrying can't fix a refusal such as `unauthenticated`, and would hold up its redirect for
// seconds, so only a Server that couldn't be reached is tried again. A failed `fetch` arrives as
// `unknown`, the same as any other error that isn't from the Server.
/** Whether to try again after `failures` failed attempts: only while the Server can't be reached. */
export function retryIfUnreachable(failures: number, error: Error) {
  return failures < 3 && (isCode(error, Code.Unavailable) || isCode(error, Code.Unknown));
}
