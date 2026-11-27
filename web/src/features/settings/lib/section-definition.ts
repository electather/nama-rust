import type { LucideIcon } from "lucide-react";
import type { ComponentType } from "react";

import type { ServerDefinedSource } from "@/features/settings/lib/server-defined";
import type { Permission, User } from "@/gen/nama/v1/sign_in_pb";
import type { SettingText } from "@/shared/lib/setting-text";
import type { Section } from "@/shared/lib/settings-address";

/** What a Section every User sees names in place of a Permission. */
export const everyUser = "every User";

/** Everything Settings needs to show one Section. */
export interface SectionDefinition {
  title: string;
  /** What else search finds it by, past its title. */
  words?: string;
  /** The Permission a User needs to see it, or `everyUser`. */
  permission: Permission | typeof everyUser;
  icon: LucideIcon;
  /** What its pane says it couldn't load, as "the Jobs". */
  couldntLoad: string;
  /** What its rows say, as the label maps they read, for search to find them by. */
  labels: Record<string, SettingText>;
  /** Where search finds the settings the Server defines in it. */
  serverDefined?: ServerDefinedSource;
  pane: ComponentType<{ user: User | undefined }>;
  /** Its header's crumbs, for a Section that opens one of its things in place of its list. */
  crumbs?: ComponentType;
  /** What acts on its whole pane, beside the close button. */
  action?: ComponentType;
}

/** A heading of the sidebar and the Sections under it, in the order they're listed. */
export interface SectionGroupDefinition {
  label: string;
  sections: Partial<Record<Section, SectionDefinition>>;
}
