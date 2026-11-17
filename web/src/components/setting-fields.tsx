import { create } from "@bufbuild/protobuf";
import { useId, useState } from "react";

import { FormField } from "@/components/form-field";
import { LibraryPicker } from "@/components/library-picker";
import { ChoiceInput } from "@/components/settings-test";
import { Button } from "@/components/ui/button";
import {
  type Choice,
  LibraryIdsSchema,
  type Setting,
  SettingKind,
  type SettingValue,
} from "@/gen/nama/v1/plugin_pb";
import { formText } from "@/lib/forms";

/**
 * One setting's input, by its kind. Shared by the Admin's Plugin settings and a User's Personal
 * settings, which a Plugin's Manifest declares alike.
 */
export function SettingInput({
  setting,
  choices,
  error,
  checksRequired,
}: {
  setting: Setting;
  /** A CHOICE setting's options, once a settings test offers them. Never for Personal settings. */
  choices?: Choice[] | undefined;
  error: string | undefined;
  /** Whether a required setting must be filled in. Not for a disabled Plugin, checked when enabled. */
  checksRequired: boolean;
}) {
  if (setting.kind === SettingKind.CHOICE) {
    const required = checksRequired && setting.required;
    // A new element once the choices come, so it starts on the stored one.
    return (
      <ChoiceInput key={String(choices !== undefined)} {...{ setting, choices, error, required }} />
    );
  }
  if (setting.kind === SettingKind.LIBRARIES) {
    return <LibraryPicker setting={setting} hint={hint(setting)} error={error} />;
  }
  return <OwnInput setting={setting} error={error} checksRequired={checksRequired} />;
}

/** The input of a setting whose value is typed or ticked on its own, with no list to pick from. */
function OwnInput({
  setting,
  error,
  checksRequired,
}: {
  setting: Setting;
  error: string | undefined;
  checksRequired: boolean;
}) {
  if (setting.kind === SettingKind.BOOL) return <BoolInput setting={setting} />;
  if (setting.secretSet) return <SetSecret setting={setting} error={error} />;
  const required = checksRequired && blankIsMissing(setting);
  return <TypedInput setting={setting} error={error} required={required} />;
}

/** A text, secret, number or URL setting's input. A secret's value never comes back, so it starts blank. */
function TypedInput({
  setting,
  error,
  required,
  focused,
}: {
  setting: Setting;
  error: string | undefined;
  required: boolean;
  /** Whether it takes the focus as it appears, in place of the button that was pressed. */
  focused?: boolean;
}) {
  return (
    <FormField
      label={setting.label}
      name={setting.key}
      {...inputs[setting.kind]}
      min={setting.minValue?.toString()}
      max={setting.maxValue?.toString()}
      autoComplete="off"
      spellCheck={false}
      // oxlint-disable-next-line jsx-a11y/no-autofocus -- only set when pressing Replace swaps it in for the button, whose focus would otherwise be lost
      autoFocus={focused}
      defaultValue={valueText(setting.value)}
      required={required}
      hint={hint(setting)}
      error={error}
    />
  );
}

/** The input attributes of each kind of setting that isn't plain text or a bool. */
const inputs: Partial<Record<SettingKind, { type: string; step?: number }>> = {
  [SettingKind.SECRET]: { type: "password" },
  [SettingKind.INTEGER]: { type: "number", step: 1 },
  [SettingKind.URL]: { type: "url" },
};

/** Whether leaving `setting` blank leaves it without a value: it has no default or stored secret. */
function blankIsMissing({ required, defaultValue, secretSet }: Setting) {
  return required && !defaultValue && !secretSet;
}

/** A secret that is set: said to be set, with a button to type a new one in its place. */
function SetSecret({ setting, error }: { setting: Setting; error: string | undefined }) {
  const [replacing, setReplacing] = useState(false);
  // A refusal naming the secret needs its input to show under.
  if (replacing || error !== undefined) {
    return <TypedInput setting={setting} error={error} required={false} focused={replacing} />;
  }
  return (
    <div className="flex items-start justify-between gap-4">
      <div>
        <p className="text-sm font-medium">{setting.label}</p>
        <p className="text-sm text-pretty text-muted-foreground empty:hidden">
          {setting.description}
        </p>
      </div>
      {/* A ghost button is pulled back by its own padding, so its words line up with the inputs. */}
      <div className="-me-4 flex shrink-0 items-center gap-2">
        <span className="text-sm">Set</span>
        <Button
          type="button"
          variant="ghost"
          aria-label={`Replace ${setting.label}`}
          onClick={() => {
            setReplacing(true);
          }}
        >
          Replace
        </Button>
      </div>
    </div>
  );
}

function BoolInput({ setting }: { setting: Setting }) {
  const hintId = useId();
  const shown = setting.value ?? setting.defaultValue;
  return (
    <div className="flex flex-col gap-2">
      <label className="flex items-center gap-3 text-sm font-medium">
        <input
          type="checkbox"
          name={setting.key}
          defaultChecked={shown?.value.case === "boolValue" && shown.value.value}
          aria-describedby={hintId}
          className="size-4 accent-foreground"
        />
        {setting.label}
      </label>
      <p id={hintId} className="text-sm text-pretty text-muted-foreground empty:hidden">
        {setting.description}
      </p>
    </div>
  );
}

/** What a setting is for, then what leaving it blank does. */
function hint({ description, required, defaultValue, secretSet }: Setting) {
  const blank = defaultValue
    ? `Blank uses ${valueText(defaultValue)}.`
    : secretSet
      ? "Leave it blank to keep the one that’s set."
      : required && "Required.";
  return [description, blank].filter(Boolean).join(" ");
}

function valueText(setting: SettingValue | undefined) {
  if (setting?.value.case === undefined) return "";
  if (setting.value.case === "libraryIds") return setting.value.value.ids.join(",");
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

function valueTyped(typed: FormData, key: string, kind: SettingKind): SettingValue["value"] {
  // An unticked checkbox isn't in the form data at all.
  if (kind === SettingKind.BOOL) return { case: "boolValue", value: typed.has(key) };
  if (kind === SettingKind.LIBRARIES) return librariesTyped(typed, key);
  return textTyped(typed, key, kind);
}

/** The setting `key` of `kind` as typed into a text input, a number for an INTEGER one. */
function textTyped(typed: FormData, key: string, kind: SettingKind): SettingValue["value"] {
  const text = formText(typed, key);
  if (text === "") return { case: undefined };
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
