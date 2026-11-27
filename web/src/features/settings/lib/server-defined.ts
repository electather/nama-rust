import { qualityLabel } from "@/features/settings/lib/default-quality";
import type { ListDefaultQualitiesResponse } from "@/gen/nama/v1/request_pb";
import type { HomeLayout } from "@/gen/nama/v1/server_pb";
import type { ListPersonalSettingsResponse } from "@/gen/nama/v1/user_pb";
import { rowName, rowSetting } from "@/shared/lib/home-layout";
import type { SettingText } from "@/shared/lib/setting-text";

/** Where search finds the settings the Server defines, as a Section's definition names it. */
export type ServerDefinedSource =
  | "personal-settings"
  | "default-qualities"
  | "home-rows"
  | "default-home-rows";

/** A setting the Server defines: search finds it in the Section whose definition names `source`. */
export interface ServerDefinedSetting {
  source: ServerDefinedSource;
  key: string;
  text: SettingText;
}

/** The key `?setting=` finds a Plugin's Personal setting `key` by. */
export function personalSetting(pluginId: string, key: string) {
  return `personal.${pluginId}.${key}`;
}

/** The key `?setting=` finds a Fulfiller's default Quality by. */
export function qualitySetting(pluginId: string) {
  return `quality.${pluginId}`;
}

/** Each Personal setting `listed`, found by its Plugin's name too. None before they load. */
export function personalFindables(
  listed: ListPersonalSettingsResponse | undefined,
): ServerDefinedSetting[] {
  return (listed?.plugins ?? []).flatMap((plugin) =>
    plugin.settings.map((setting) => ({
      source: "personal-settings",
      key: personalSetting(plugin.pluginId, setting.key),
      text: { label: setting.label, hint: setting.description, words: plugin.name },
    })),
  );
}

/** Each Fulfiller's default Quality `listed`. None before they load. */
export function qualityFindables(
  listed: ListDefaultQualitiesResponse | undefined,
): ServerDefinedSetting[] {
  return (listed?.defaults ?? []).map((fulfiller) => ({
    source: "default-qualities",
    key: qualitySetting(fulfiller.pluginId),
    text: { label: qualityLabel(fulfiller), words: "requests" },
  }));
}

/** Each Home row of the layout `got`, from `source`. None before it loads. */
export function homeRowFindables(
  source: "home-rows" | "default-home-rows",
  got: { layout?: HomeLayout | undefined } | undefined,
): ServerDefinedSetting[] {
  return (got?.layout?.rows ?? []).map((row) => ({
    source,
    key: rowSetting(row),
    text: { label: rowName(row).label, words: "home row" },
  }));
}
