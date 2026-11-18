import { useMutation } from "@connectrpc/connect-query";
import { useState } from "react";

import { PluginService, type SettingChoices } from "@/gen/nama/v1/plugin_pb";

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
