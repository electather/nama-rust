import { useMutation } from "@connectrpc/connect-query";
import { useState } from "react";

import { type Plugin, PluginService, type SettingChoices } from "@/gen/nama/v1/plugin_pb";
import { Button } from "@/shared/ui/button";

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
  if (!plugin.checksSettings) {
    return null;
  }
  return (
    <Button
      type="button"
      variant="ghost"
      disabled={pending}
      onClick={(event) => {
        const { form } = event.currentTarget;
        if (form) {
          onTest(new FormData(form));
        }
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
