import { type Code, ConnectError } from "@connectrpc/connect";

/** Whether `error`, from an RPC or not, carries the Connect `code`. */
export function isCode(error: unknown, code: Code) {
  return ConnectError.from(error).code === code;
}
