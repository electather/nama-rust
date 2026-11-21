import { create } from "@bufbuild/protobuf";

import {
  LibraryIdsSchema,
  type Setting,
  SettingKind,
  type SettingValue,
} from "@/gen/nama/v1/plugin_pb";
import { formText } from "@/shared/lib/forms";

/** The form field naming each secret Forget was pressed for, which `forgottenTyped` reads. */
export const forgottenField = "forgottenSecrets";

/** What a set secret's buttons do: type a new one in its place, or forget it. */
export type SecretAction = "Replace" | "Forget";

/** Whether leaving `setting` blank leaves it without a value: it has no default or stored secret. */
export function blankIsMissing({ required, defaultValue, secretSet }: Setting) {
  return required && !defaultValue && !secretSet;
}

/** What a setting is for, then what leaving it blank does: to a set secret, `secretBlank`. */
export function hint(setting: Setting, secretBlank: "keep" | "forget" = "keep") {
  return [setting.description, blankHint(setting, secretBlank)].filter(Boolean).join(" ");
}

function blankHint({ required, defaultValue, secretSet }: Setting, secretBlank: "keep" | "forget") {
  if (defaultValue) {
    return `Blank uses ${valueText(defaultValue)}.`;
  }
  if (secretSet) {
    return `Leave it blank to ${secretBlank} the one that’s set.`;
  }
  return required ? "Required." : "";
}

/** A setting's value as an input shows it. */
export function valueText(setting: SettingValue | undefined) {
  if (setting?.value.case === undefined) {
    return "";
  }
  if (setting.value.case === "libraryIds") {
    return setting.value.value.ids.join(",");
  }
  if (setting.value.case === "texts") {
    return setting.value.value.values.join(", ");
  }
  return String(setting.value.value);
}

/** The settings typed in `typed`, one per setting of `settings`. A blank one is left out. */
export function settingsTyped(typed: FormData, settings: Setting[]) {
  return Object.fromEntries(
    settings
      .map(({ key, kind }) => [key, { value: valueTyped(typed, key, kind) }] as const)
      .filter(([, { value }]) => value.case !== undefined),
  );
}

/** The keys of the set secrets Forget was pressed for. */
export function forgottenTyped(typed: FormData) {
  return typed.getAll(forgottenField).filter((key) => typeof key === "string");
}

function valueTyped(typed: FormData, key: string, kind: SettingKind): SettingValue["value"] {
  // An unticked checkbox isn't in the form data at all.
  if (kind === SettingKind.BOOL) {
    return { case: "boolValue", value: typed.has(key) };
  }
  if (kind === SettingKind.LIBRARIES) {
    return librariesTyped(typed, key);
  }
  return textTyped(typed, key, kind);
}

/** The setting `key` of `kind` as typed into a text input, a number for an INTEGER one. */
function textTyped(typed: FormData, key: string, kind: SettingKind): SettingValue["value"] {
  const text = formText(typed, key);
  if (text === "") {
    return { case: undefined };
  }
  // The input only takes whole numbers (`step` 1), but may write one as `1e3` or `1.0`, which
  // `BigInt` alone refuses. ponytail: past 2^53 a number loses precision on the way.
  return kind === SettingKind.INTEGER
    ? { case: "integerValue", value: BigInt(Number(text)) }
    : { case: "textValue", value: text };
}

/** The Libraries ticked for the setting `key`, by id. None ticked is blank. */
function librariesTyped(typed: FormData, key: string): SettingValue["value"] {
  const ids = typed.getAll(key).flatMap((id) => (typeof id === "string" ? [BigInt(id)] : []));
  return ids.length === 0
    ? { case: undefined }
    : { case: "libraryIds", value: create(LibraryIdsSchema, { ids }) };
}
