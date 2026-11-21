import type { ConnectError } from "@connectrpc/connect";

import { refusal } from "@/shared/lib/forms";

/** `rows` with the row at `from` put before the one at `before`, or last for `rows.length`. */
export function moved<T>(rows: readonly T[], from: number, before: number) {
  const row = rows[from];
  if (row === undefined) {
    return [...rows];
  }
  const rest = rows.filter((_, index) => index !== from);
  rest.splice(before > from ? before - 1 : before, 0, row);
  return rest;
}

/** Why the Server refused to save the layout, or `undefined` while it hasn't. */
export function saveRefused(error: ConnectError | null) {
  return error === null ? undefined : refusal(error, "save the Home layout", {}).form;
}
