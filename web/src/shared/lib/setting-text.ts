import type { Permission } from "@/gen/nama/v1/sign_in_pb";

/**
 * What a setting's row says, from its Section's label map, and what else Settings' search finds it
 * by. The row reads it from the map, so the search can't drift from the row.
 */
export interface SettingText {
  label: string;
  hint?: string;
  /** Words the row doesn't say that someone may search for it by. */
  words?: string;
  /** The setting whose row shows this one's, for a row shown only sometimes. */
  shownBy?: string;
  /** The Permission it's shown only with, for a row not every User of its Section has. */
  needs?: Permission;
}

/** What a row takes from `texts` for the setting `key`: its label, its hint, and `key` to find it by. */
export function rowText<Key extends string>(texts: Record<Key, SettingText>, key: Key) {
  const { label, hint } = texts[key];
  return { setting: key, label, hint };
}
