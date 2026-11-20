import {
  Activity,
  Bell,
  CalendarClock,
  Captions,
  Database,
  Film,
  Fingerprint,
  KeyRound,
  Link,
  Rows3,
  ScanSearch,
  Server,
  Tv,
} from "lucide-react";

import { ExtensionPoint } from "@/gen/nama/v1/plugin_pb";
import type { Shown } from "@/shared/components/state-label";

/** Each Extension Point in plain words, with the icon the web picks for it. */
export const points: Partial<Record<ExtensionPoint, Shown>> = {
  [ExtensionPoint.MOVIE_FULFILLER]: { label: "Fulfiller for Movies", Icon: Film },
  [ExtensionPoint.SHOW_FULFILLER]: { label: "Fulfiller for Shows", Icon: Tv },
  [ExtensionPoint.NOTIFIER]: { label: "Notifier", Icon: Bell },
  [ExtensionPoint.CONNECTOR]: { label: "Connector", Icon: Link },
  [ExtensionPoint.AUTHENTICATOR]: { label: "Authenticator", Icon: KeyRound },
  [ExtensionPoint.TITLE_SOURCE]: { label: "Title source", Icon: ScanSearch },
  [ExtensionPoint.EVENTS_OBSERVER]: { label: "Observes Events", Icon: Activity },
  [ExtensionPoint.SERVER_STARTED_OBSERVER]: { label: "Observes Server start", Icon: Server },
  [ExtensionPoint.RECOGNISER]: { label: "Recogniser", Icon: Fingerprint },
  [ExtensionPoint.BLOB_STORE]: { label: "Blob store", Icon: Database },
  [ExtensionPoint.AUTOMATION]: { label: "Automation", Icon: CalendarClock },
  [ExtensionPoint.HOME_ROW]: { label: "Home rows", Icon: Rows3 },
  [ExtensionPoint.SUBTITLE_SOURCE]: { label: "Subtitle source", Icon: Captions },
};

/** What a point's failing calls leave undone, for the points where that isn't plain. */
export const failureMeans: Partial<Record<ExtensionPoint, string>> = {
  [ExtensionPoint.RECOGNISER]:
    "Until it answers, the Items it is asked about stay undecided and are tried again at the next Scan; the rest of Matching goes on.",
  [ExtensionPoint.HOME_ROW]:
    "Users go on seeing the rows it last gave, and a visit to home 15 minutes or more later asks it again.",
};
