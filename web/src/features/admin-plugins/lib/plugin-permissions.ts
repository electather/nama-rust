import { PluginPermission } from "@/gen/nama/v1/plugin_pb";

/** Each Plugin Permission in plain words, with what granting it lets the Plugin do. */
export const pluginPermissions: Partial<
  Record<PluginPermission, { label: string; about: string }>
> = {
  [PluginPermission.STORE]: {
    label: "Keep its own data",
    about: "It keeps notes in the Server, such as which titles it has already looked at.",
  },
  [PluginPermission.READ_LIBRARIES]: {
    label: "Read Libraries",
    about: "It sees every Library and the Movies and Episodes in it.",
  },
  [PluginPermission.READ_WATCH_STATES]: {
    label: "Read Watch State",
    about: "It sees what every User has watched, and when.",
  },
  [PluginPermission.DELETE_MEDIA]: {
    label: "Delete media",
    about: "It can delete Movies and Episodes, with their files on disk, for good.",
  },
  [PluginPermission.READ_USERS]: {
    label: "Read Users",
    about: "It sees every User's id and display name, and nothing else about them.",
  },
  [PluginPermission.READ_REQUESTS]: {
    label: "Read Requests",
    about:
      "It sees which Users asked for each Movie and Episode, and nothing else about their Requests.",
  },
  [PluginPermission.READ_WATCHLISTS]: {
    label: "Read Watchlists",
    about: "It sees the Movies and Shows on every User's Watchlist.",
  },
  [PluginPermission.FORGET_HOME_ROWS]: {
    label: "Forget Home rows",
    about:
      "It deletes the Home rows and Featured suggestions kept for Users who stopped opening home. They get fresh ones when they come back.",
  },
};
