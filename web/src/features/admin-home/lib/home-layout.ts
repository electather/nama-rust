import type { ConnectError } from "@connectrpc/connect";

import type { HomeLayoutRow } from "@/gen/nama/v1/server_pb";
import { refusal } from "@/shared/lib/forms";

/** What the Home page calls a row of a Home layout, and the Plugin it's from, if any. */
export interface RowName {
  label: string;
  from?: string;
}

/** What tells `row` from the layout's other rows. */
export function rowKey({ row }: HomeLayoutRow) {
  if (row.case === "plugin") {
    return `plugin/${row.value.pluginId}/${row.value.key}`;
  }
  if (row.case === "pluginUsersRows") {
    return `users/${row.value}`;
  }
  return `kernel/${String(row.value)}`;
}

/** What the Home page calls `row`, with the Plugin it's from by the name `names` gives it. */
export function rowName({ row }: HomeLayoutRow, names: Map<string, string>): RowName {
  if (row.case === "plugin") {
    return { label: row.value.heading, from: fromPlugin(row.value.pluginId, names) };
  }
  if (row.case === "pluginUsersRows") {
    return { label: "Each User’s own rows", from: fromPlugin(row.value, names) };
  }
  // Continue Watching is the only Kernel row.
  return { label: "Continue Watching" };
}

function fromPlugin(plugin: string, names: Map<string, string>) {
  return `From ${names.get(plugin) ?? plugin}`;
}

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
