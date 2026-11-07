import { createConnectQueryKey, useMutation } from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";
import { useId, useState } from "react";

import { FormEnd, FormField } from "@/components/form-field";
import { Button } from "@/components/ui/button";
import {
  type Plugin,
  PluginService,
  type Setting,
  SettingKind,
  type SettingValue,
} from "@/gen/nama/v1/plugin_pb";
import { RefusalReason } from "@/gen/nama/v1/refusal_pb";
import { formText, onSubmitted, refusal, refusalOf } from "@/lib/forms";

const pluginsKey = createConnectQueryKey({ schema: PluginService, cardinality: "finite" });

/** The form an Admin changes `plugin`'s settings with, checked against its schema as they type. */
export function PluginSettings({ plugin }: { plugin: Plugin }) {
  const queryClient = useQueryClient();
  const update = useMutation(PluginService.method.updatePluginSettings, {
    onSuccess: () => queryClient.invalidateQueries({ queryKey: pluginsKey }),
  });
  const errors = update.error ? settingErrors(update.error) : {};
  return (
    <form
      className="mt-4 flex max-w-md flex-col gap-5"
      onSubmit={onSubmitted((typed) => {
        update.mutate({ id: plugin.id, settings: settingsTyped(typed, plugin.settings) });
      })}
    >
      {plugin.settings.map((setting) => (
        <SettingInput key={setting.key} setting={setting} error={errors[setting.key]} />
      ))}
      <FormEnd
        action={update.isPending ? "Saving…" : "Save"}
        variant="ghost"
        pending={update.isPending}
        done={update.isSuccess ? saved(update.data.plugin) : undefined}
        refusal={errors["form"]}
      />
    </form>
  );
}

function SettingInput({ setting, error }: { setting: Setting; error: string | undefined }) {
  if (setting.kind === SettingKind.BOOL) return <BoolInput setting={setting} />;
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

/** Why the Server refused the settings: next to the setting its Refusal names, or for the form. */
function settingErrors(error: unknown): Record<string, string | undefined> {
  const { reason, field } = refusalOf(error);
  const key = field.replace(/^settings\./u, "");
  if (key === field) {
    const { form } = refusal(
      error,
      "save the settings",
      {},
      { [RefusalReason.PLUGIN_NOT_FOUND]: { form: "This Plugin isn’t on the Server any more." } },
    );
    return { form };
  }
  return {
    [key]:
      reason === RefusalReason.FIELD_REQUIRED
        ? "This setting is required."
        : "This setting can’t take that value.",
  };
}

/** What saving did, by whether `plugin` runs with the new settings now. */
function saved(plugin: Plugin | undefined) {
  return plugin?.enabled === true
    ? "Saved, and in use now."
    : "Saved. They’re used once the Plugin is enabled.";
}
