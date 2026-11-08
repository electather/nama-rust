import { useMutation } from "@connectrpc/connect-query";
import { useState } from "react";

import { FormSelect } from "@/components/form-field";
import { Button } from "@/components/ui/button";
import {
  type Plugin,
  PluginService,
  type Setting,
  type SettingChoices,
} from "@/gen/nama/v1/plugin_pb";

/**
 * Testing a Plugin's settings: the test itself, and the choices for each choice setting that the
 * last test offered, none once one fails.
 */
export function useSettingsTest() {
  // Kept apart from the test's own state, so a save that resets it keeps them.
  const [choices, setChoices] = useState<Record<string, SettingChoices>>();
  const test = useMutation(PluginService.method.testPluginSettings, {
    onSuccess: (result) => {
      setChoices(result.choices);
    },
    onError: () => {
      setChoices(undefined);
    },
  });
  return { choices, test };
}

/** The button that hands `onTest` the settings typed in its form, for a `plugin` with a check. */
export function TestButton({
  plugin,
  pending,
  onTest,
}: {
  plugin: Plugin;
  pending: boolean;
  onTest: (typed: FormData) => void;
}) {
  if (!plugin.checksSettings) return null;
  return (
    <Button
      type="button"
      variant="ghost"
      disabled={pending}
      onClick={(event) => {
        const { form } = event.currentTarget;
        if (form) onTest(new FormData(form));
      }}
    >
      {pending ? "Testing…" : "Test"}
    </Button>
  );
}

/** What testing did, once it passed. */
export function testPassed(passed: boolean) {
  return passed ? "Test passed. You can choose from what it offers now." : undefined;
}

/**
 * A choice setting: a dropdown of what the Plugin's last passing test offered, locked until a test
 * passes, as only the outside service knows what may be picked.
 */
export function ChoiceInput({
  setting,
  choices,
  error,
}: {
  setting: Setting;
  choices: SettingChoices | undefined;
  error: string | undefined;
}) {
  const stored = setting.value?.value.case === "textValue" ? setting.value.value.value : "";
  if (choices === undefined) {
    return (
      <>
        <FormSelect
          label={setting.label}
          name={setting.key}
          disabled
          hint={[setting.description, "Test the settings to choose from what it offers."].join(" ")}
          error={error}
        >
          <option>{stored === "" ? "Not chosen" : stored}</option>
        </FormSelect>
        {/* A disabled dropdown isn't sent, so this sends the stored choice as it was. */}
        <input type="hidden" name={setting.key} value={stored} />
      </>
    );
  }
  return (
    <FormSelect
      label={setting.label}
      name={setting.key}
      defaultValue={stored}
      required={setting.required}
      hint={setting.description}
      error={error}
    >
      <option value="">Pick one</option>
      {choices.choices.map(({ value, label }) => (
        <option key={value} value={value}>
          {label}
        </option>
      ))}
    </FormSelect>
  );
}
