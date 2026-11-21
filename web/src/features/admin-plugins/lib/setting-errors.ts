import { refusedSettings } from "@/shared/lib/setting-refusals";

/**
 * Why the Server refused to save or test the settings, by whichever it refused. A `null` error
 * refused nothing.
 */
export function settingErrors(saving: unknown, testing: unknown) {
  if (saving !== null) {
    return refusedSettings(saving, "save the settings");
  }
  return testing === null ? {} : refusedSettings(testing, "test the settings");
}
