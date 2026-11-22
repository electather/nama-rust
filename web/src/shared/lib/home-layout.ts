import type { HomeLayoutRow } from "@/gen/nama/v1/server_pb";
import type { SettingText } from "@/shared/lib/setting-text";

/** What the built-in rows of a Home layout's list say, above its Home rows. */
export const homeLabels = {
  featured: { label: "Featured", hint: "Always at the top", words: "hero spotlight" },
} satisfies Record<string, SettingText>;

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

/** The key `?setting=` finds `row` by in a Home layout's list. */
export function rowSetting(row: HomeLayoutRow) {
  return `row.${rowKey(row)}`;
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

/** What assistive technology announces once `row` has moved to its place in `rows`. */
export function moveAnnouncement(
  row: HomeLayoutRow,
  rows: HomeLayoutRow[],
  names?: Map<string, string>,
) {
  const { label } = rowName(row, names);
  return `${label} moved to place ${rows.indexOf(row) + 1} of ${rows.length}.`;
}

function fromPlugin(plugin: string, names?: Map<string, string>) {
  return names && `From ${names.get(plugin) ?? plugin}`;
}
