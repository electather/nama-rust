import type { HomeLayoutRow } from "@/gen/nama/v1/server_pb";

/** What a Home layout's list calls a row, and what it says under it, if anything. */
export interface RowName {
  label: string;
  from?: string | undefined;
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

/**
 * What a Home layout's list calls `row`, with the Plugin it's from by the name `names` gives it.
 * Without `names`, which only admins can list, a Plugin's row is called by its heading alone.
 */
export function rowName({ row }: HomeLayoutRow, names?: Map<string, string>): RowName {
  if (row.case === "plugin") {
    return { label: row.value.heading, from: fromPlugin(row.value.pluginId, names) };
  }
  if (row.case === "pluginUsersRows") {
    return { label: "Each User’s own rows", from: fromPlugin(row.value, names) };
  }
  // Continue Watching is the only Kernel row.
  return { label: "Continue Watching" };
}

function fromPlugin(plugin: string, names?: Map<string, string>) {
  return names && `From ${names.get(plugin) ?? plugin}`;
}
