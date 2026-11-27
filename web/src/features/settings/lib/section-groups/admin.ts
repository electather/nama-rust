import { Cpu, DatabaseBackup, HardDrive, Puzzle, Rows3, Users } from "lucide-react";

import { backupLabels } from "@/features/admin-backups/lib/backup-settings";
import { serverPlaybackLabels } from "@/features/admin-playback/lib/playback-labels";
import { BackupsPane } from "@/features/settings/components/backups-pane";
import { DefaultHomePane } from "@/features/settings/components/default-home-pane";
import { PluginCrumbs } from "@/features/settings/components/plugin-crumbs";
import { PluginsPane } from "@/features/settings/components/plugins-pane";
import { ServerPane } from "@/features/settings/components/server-pane";
import { ServerPlaybackPane } from "@/features/settings/components/server-playback-pane";
import { UserCrumbs } from "@/features/settings/components/user-crumbs";
import { UsersPane } from "@/features/settings/components/users-pane";
import { userLabels } from "@/features/settings/lib/pane-labels";
import type { SectionGroupDefinition } from "@/features/settings/lib/section-definition";
import { serverLabels } from "@/features/settings/lib/server-values";
import { Permission } from "@/gen/nama/v1/sign_in_pb";
import { homeLabels } from "@/shared/lib/home-layout";

/** The Server's Sections of how it runs and who uses it. */
export const admin = {
  label: "Admin",
  sections: {
    server: {
      title: "Server",
      permission: Permission.MANAGE_SETTINGS,
      icon: HardDrive,
      couldntLoad: "the Server settings",
      labels: serverLabels,
      pane: ServerPane,
    },
    users: {
      title: "Users",
      permission: Permission.MANAGE_USERS,
      icon: Users,
      couldntLoad: "the Users",
      labels: userLabels,
      pane: UsersPane,
      crumbs: UserCrumbs,
    },
    backups: {
      title: "Backups",
      permission: Permission.ADMIN,
      icon: DatabaseBackup,
      couldntLoad: "the Backups",
      labels: backupLabels,
      pane: BackupsPane,
    },
    "default-home": {
      title: "Default Home",
      permission: Permission.MANAGE_SETTINGS,
      icon: Rows3,
      couldntLoad: "the default Home layout",
      labels: homeLabels,
      serverDefined: "default-home-rows",
      pane: DefaultHomePane,
    },
    "server-playback": {
      title: "Server Playback",
      permission: Permission.ADMIN,
      icon: Cpu,
      couldntLoad: "the Server’s Playback settings",
      labels: serverPlaybackLabels,
      pane: ServerPlaybackPane,
    },
    plugins: {
      title: "Plugins",
      permission: Permission.MANAGE_SETTINGS,
      icon: Puzzle,
      couldntLoad: "the Plugins",
      labels: {},
      pane: PluginsPane,
      crumbs: PluginCrumbs,
    },
  },
} satisfies SectionGroupDefinition;
