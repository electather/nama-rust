import { useDeletePersonalSettings } from "@/features/settings/data/use-delete-personal-settings";
import { useUpdatePersonalSettings } from "@/features/settings/data/use-update-personal-settings";
import { settingErrors } from "@/features/settings/lib/personal-errors";

/**
 * Saving and forgetting the Plugin `name`'s Personal settings, each listing them again once it's
 * done, and why the Server refused the last one, by setting.
 */
export function usePersonalChanges(name: string) {
  const update = useUpdatePersonalSettings();
  const forget = useDeletePersonalSettings();
  return { update, forget, errors: settingErrors(update.error, forget.error, name) };
}
