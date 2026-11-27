import { Bell, Film, House, SlidersHorizontal } from "lucide-react";

import { ConnectionsPane } from "@/features/settings/components/connections-pane";
import { HomePane } from "@/features/settings/components/home-pane";
import { PlaybackPane } from "@/features/settings/components/playback-pane";
import { RequestsPane } from "@/features/settings/components/requests-pane";
import { ResetHomeLayout } from "@/features/settings/components/reset-home-layout";
import { browserPushText, notificationLabels } from "@/features/settings/lib/notification-labels";
import { playbackLabels } from "@/features/settings/lib/playback-values";
import { type SectionGroupDefinition, everyUser } from "@/features/settings/lib/section-definition";
import { homeLabels } from "@/shared/lib/home-layout";

/** The Server's Sections of how the User likes things. */
export const preferences = {
  label: "Preferences",
  sections: {
    playback: {
      title: "Playback",
      permission: everyUser,
      icon: SlidersHorizontal,
      couldntLoad: "your Playback settings",
      labels: playbackLabels,
      pane: PlaybackPane,
    },
    home: {
      title: "Home",
      permission: everyUser,
      icon: House,
      couldntLoad: "your Home layout",
      labels: homeLabels,
      serverDefined: "home-rows",
      pane: HomePane,
      action: ResetHomeLayout,
    },
    requests: {
      title: "Requests",
      words: "requests quality withdraw approve decline pending followed shows",
      permission: everyUser,
      icon: Film,
      couldntLoad: "your Requests",
      labels: {},
      serverDefined: "default-qualities",
      pane: RequestsPane,
    },
    connections: {
      title: "Connections",
      permission: everyUser,
      icon: Bell,
      couldntLoad: "your Connections",
      labels: { ...notificationLabels, "browser-push": browserPushText },
      serverDefined: "personal-settings",
      pane: ConnectionsPane,
    },
  },
} satisfies SectionGroupDefinition;
