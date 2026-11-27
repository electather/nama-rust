import { CalendarClock, MonitorPlay } from "lucide-react";

import { JobCrumbs } from "@/features/settings/components/job-crumbs";
import { JobRunNow } from "@/features/settings/components/job-run-now";
import { JobsPane } from "@/features/settings/components/jobs-pane";
import { NowPlayingPane } from "@/features/settings/components/now-playing-pane";
import type { SectionGroupDefinition } from "@/features/settings/lib/section-definition";
import { Permission } from "@/gen/nama/v1/sign_in_pb";

/** The Server's Sections of what it's doing. */
export const activity = {
  label: "Activity",
  sections: {
    "now-playing": {
      title: "Now Playing",
      words: "now playing watching streams sessions stop transcode",
      permission: Permission.ADMIN,
      icon: MonitorPlay,
      couldntLoad: "what’s playing",
      labels: {},
      pane: NowPlayingPane,
    },
    jobs: {
      title: "Jobs",
      words: "jobs schedule run tasks upkeep",
      permission: Permission.ADMIN,
      icon: CalendarClock,
      couldntLoad: "the Jobs",
      labels: {},
      pane: JobsPane,
      crumbs: JobCrumbs,
      action: JobRunNow,
    },
  },
} satisfies SectionGroupDefinition;
