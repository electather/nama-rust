import { createConnectQueryKey, useMutation } from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";
import { useId, useState } from "react";

import { FormEnd, FormField } from "@/components/form-field";
import { ChoiceInput, TestButton, testPassed, useSettingsTest } from "@/components/settings-test";
import { Button } from "@/components/ui/button";
import {
  type Choice,
  type Plugin,
  PluginService,
  type Setting,
  SettingKind,
  type SettingValue,
} from "@/gen/nama/v1/plugin_pb";
import { RefusalReason } from "@/gen/nama/v1/refusal_pb";
import { formText, onSubmitted, refusal, refusalOf, serverMessage } from "@/lib/forms";

const pluginsKey = createConnectQueryKey({ schema: PluginService, cardinality: "finite" });

/**
 * The form an Admin changes `plugin`'s settings with, checked against its schema as they type. A
 * Plugin with a settings check can test what was typed, which offers the choices for its choice
 * settings.
 */
export function PluginSettings({ plugin }: { plugin: Plugin }) {
  const queryClient = useQueryClient();
  const update = useMutation(PluginService.method.updatePluginSettings, {
    onSuccess: () => queryClient.invalidateQueries({ queryKey: pluginsKey }),
  });
  const { choices, test } = useSettingsTest();
  // One message at a time: each resets the other.
  const errors = settingErrors(update.error, test.error);
  return (
    <form
      className="mt-4 flex max-w-md flex-col gap-5"
      onSubmit={onSubmitted((typed) => {
        test.reset();
        update.mutate({ id: plugin.id, settings: settingsTyped(typed, plugin.settings) });
      })}
    >
      {plugin.settings.map((setting) => (
        <SettingInput
          key={setting.key}
          setting={setting}
          // A passing test that offered nothing for a setting still unlocks it, empty.
          choices={choices && (choices[setting.key]?.choices ?? [])}
          error={errors[setting.key]}
        />
      ))}
      <FormEnd
        action={update.isPending ? "Saving…" : "Save"}
        variant="ghost"
        pending={update.isPending}
        done={update.isSuccess ? saved(update.data.plugin) : testPassed(test.isSuccess)}
        refusal={errors["form"]}
      >
        <TestButton
          plugin={plugin}
          pending={test.isPending}
          onTest={(typed) => {
            update.reset();
            test.mutate({ id: plugin.id, settings: settingsTyped(typed, plugin.settings) });
          }}
        />
      </FormEnd>
    </form>
  );
}

function SettingInput({
  setting,
  choices,
  error,
}: {
  setting: Setting;
  choices: Choice[] | undefined;
  error: string | undefined;
}) {
  if (setting.kind === SettingKind.BOOL) return <BoolInput setting={setting} />;
  if (setting.kind === SettingKind.CHOICE) {
    // A new element once the choices come, so it starts on the stored one.
    return <ChoiceInput key={String(choices !== undefined)} {...{ setting, choices, error }} />;
  }
  if (setting.secretSet) return <SetSecret setting={setting} error={error} />;
  return <TypedInput setting={setting} error={error} />;
}

/** A text, secret or number setting's input. A secret's value never comes back, so it starts blank. */
function TypedInput({
  setting,
  error,
  focused,
}: {
  setting: Setting;
  error: string | undefined;
  /** Whether it takes the focus as it appears, in place of the button that was pressed. */
  focused?: boolean;
}) {
  return (
    <FormField
      label={setting.label}
      name={setting.key}
      {...inputs[setting.kind]}
      autoComplete="off"
      spellCheck={false}
      // oxlint-disable-next-line jsx-a11y/no-autofocus -- only set when pressing Replace swaps it in for the button, whose focus would otherwise be lost
      autoFocus={focused}
      defaultValue={valueText(setting.value)}
      required={blankIsMissing(setting)}
      hint={hint(setting)}
      error={error}
    />
  );
}

/** The input attributes of each kind of setting that isn't plain text or a bool. */
const inputs: Partial<Record<SettingKind, { type: string; step?: number }>> = {
  [SettingKind.SECRET]: { type: "password" },
  [SettingKind.INTEGER]: { type: "number", step: 1 },
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
    return <TypedInput setting={setting} error={error} focused={replacing} />;
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
  return setting?.value.case === undefined ? "" : String(setting.value.value);
}

/** The settings typed in `typed`, one per setting of `settings`. A blank one is left out. */
function settingsTyped(typed: FormData, settings: Setting[]) {
  return Object.fromEntries(
    settings
      .map(({ key, kind }) => [key, { value: valueTyped(typed, key, kind) }] as const)
      .filter(([, { value }]) => value.case !== undefined),
  );
}

function valueTyped(typed: FormData, key: string, kind: SettingKind): SettingValue["value"] {
  // An unticked checkbox isn't in the form data at all.
  if (kind === SettingKind.BOOL) return { case: "boolValue", value: typed.has(key) };
  const text = formText(typed, key);
  if (text === "") return { case: undefined };
  // The input only takes whole numbers (`step` 1), but may write one as `1e3` or `1.0`, which
  // `BigInt` alone refuses. ponytail: past 2^53 a number loses precision on the way.
  return kind === SettingKind.INTEGER
    ? { case: "integerValue", value: BigInt(Number(text)) }
    : { case: "textValue", value: text };
}

/**
 * Why the Server refused to save or test the settings, by whichever it refused. A `null` error
 * refused nothing.
 */
function settingErrors(saving: unknown, testing: unknown) {
  if (saving !== null) return refusedSettings(saving, "save the settings");
  return testing === null ? {} : refusedSettings(testing, "test the settings");
}

/** Why the Server refused the `attempt`: next to the setting its Refusal names, or for the form. */
function refusedSettings(error: unknown, attempt: string): Record<string, string | undefined> {
  const { reason, field } = refusalOf(error);
  const key = field.replace(/^settings\./u, "");
  if (key === field) {
    const { form } = refusal(
      error,
      attempt,
      {},
      {
        [RefusalReason.PLUGIN_NOT_FOUND]: { form: "This Plugin isn’t on the Server any more." },
        // The Plugin's own words.
        [RefusalReason.SETTINGS_CHECK_FAILED]: { form: serverMessage(error) },
      },
    );
    return { form };
  }
  // A failed check is in the Plugin's own words.
  if (reason === RefusalReason.SETTINGS_CHECK_FAILED) return { [key]: serverMessage(error) };
  return { [key]: settingRefused[reason] ?? "This setting can’t take that value." };
}

/** Why the Server refused a setting, by its Refusal's reason. */
const settingRefused: Partial<Record<RefusalReason, string>> = {
  [RefusalReason.FIELD_REQUIRED]: "This setting is required.",
  [RefusalReason.CHOICE_NOT_OFFERED]:
    "The Plugin doesn’t offer this choice now. Test the settings and pick again.",
};

/** What saving did, by whether `plugin` runs with the new settings now. */
function saved(plugin: Plugin | undefined) {
  return plugin?.enabled === true
    ? "Saved, and in use now."
    : "Saved. They’re used once the Plugin is enabled.";
}
