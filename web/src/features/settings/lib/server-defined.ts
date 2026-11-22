import { qualityLabel } from "@/features/settings/lib/default-quality";
import type { Findable } from "@/features/settings/lib/setting-search";
import type { ListDefaultQualitiesResponse } from "@/gen/nama/v1/request_pb";
import type { HomeLayout } from "@/gen/nama/v1/server_pb";
import type { ListPersonalSettingsResponse } from "@/gen/nama/v1/user_pb";
import { rowName, rowSetting } from "@/shared/lib/home-layout";

/** The key `?setting=` finds a Plugin's Personal setting `key` by. */
export function personalSetting(pluginId: string, key: string) {
  return `personal.${pluginId}.${key}`;
}

/** The key `?setting=` finds a Fulfiller's default Quality by. */
export function qualitySetting(pluginId: string) {
  return `quality.${pluginId}`;
}

/** Each Personal setting `listed`, found by its Plugin's name too. None before they load. */
export function personalFindables(listed: ListPersonalSettingsResponse | undefined): Findable[] {
  return (listed?.plugins ?? []).flatMap((plugin) =>
    plugin.settings.map((setting) => ({
      section: "connections",
      key: personalSetting(plugin.pluginId, setting.key),
      text: { label: setting.label, hint: setting.description, words: plugin.name },
    })),
  );
}

/** Each Fulfiller's default Quality `listed`. None before they load. */
export function qualityFindables(listed: ListDefaultQualitiesResponse | undefined): Findable[] {
  return (listed?.defaults ?? []).map((fulfiller) => ({
    section: "requests",
    key: qualitySetting(fulfiller.pluginId),
    text: { label: qualityLabel(fulfiller), words: "requests" },
  }));
}

/** Each Home row of the layout `got`, in the Home layout `section` lists. None before it loads. */
export function homeRowFindables(
  section: "home" | "default-home",
  got: { layout?: HomeLayout | undefined } | undefined,
): Findable[] {
  return (got?.layout?.rows ?? []).map((row) => ({
    section,
    key: rowSetting(row),
    text: { label: rowName(row).label, words: "home row" },
  }));
}
