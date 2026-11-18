import { refusedSettings } from "@/shared/lib/setting-refusals";

/** Why the Server refused to save or forget `name`'s settings, by whichever it refused. */
export function settingErrors(saving: unknown, forgetting: unknown, name: string) {
  if (saving !== null) {
    return refusedSettings(saving, `save your ${name} settings`);
  }
  return forgetting === null ? {} : refusedSettings(forgetting, `forget your ${name} settings`);
}
