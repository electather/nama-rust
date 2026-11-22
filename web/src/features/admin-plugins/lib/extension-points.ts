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
  Search,
  Server,
  Shapes,
  SkipForward,
  Tv,
} from "lucide-react";

import { ExtensionPoint } from "@/gen/nama/v1/plugin_pb";
import type { Shown } from "@/shared/components/state-label";

/**
 * Each Extension Point in plain words, with the icon the web picks for it and a line on what a
 * Plugin does there, in the words of `plugin.proto` and the Glossary.
 */
export const points: Partial<Record<ExtensionPoint, Shown & { means: string }>> = {
  [ExtensionPoint.MOVIE_FULFILLER]: {
    label: "Fulfiller for Movies",
    Icon: Film,
    means: "Hands Approved Requests for Movies to an outside service, such as Radarr.",
  },
  [ExtensionPoint.SHOW_FULFILLER]: {
    label: "Fulfiller for Shows",
    Icon: Tv,
    means: "Hands Approved Requests for Seasons to an outside service, such as Sonarr.",
  },
  [ExtensionPoint.NOTIFIER]: {
    label: "Notifier",
    Icon: Bell,
    means: "Delivers notifications about Requests through an outside service.",
  },
  [ExtensionPoint.CONNECTOR]: {
    label: "Connector",
    Icon: Link,
    means: "Connects Users to an outside service through a link they open.",
  },
  [ExtensionPoint.AUTHENTICATOR]: {
    label: "Authenticator",
    Icon: KeyRound,
    means: "Checks the credentials people sign in with.",
  },
  [ExtensionPoint.TITLE_SOURCE]: {
    label: "Title source",
    Icon: ScanSearch,
    means: "Searches for and describes Movies and Shows, for the Libraries that ask it.",
  },
  [ExtensionPoint.EVENTS_OBSERVER]: {
    label: "Observes Events",
    Icon: Activity,
    means: "Is told each of the Server’s Events as it happens, and can’t change it.",
  },
  [ExtensionPoint.SERVER_STARTED_OBSERVER]: {
    label: "Observes Server start",
    Icon: Server,
    means: "Is told each time the Server starts.",
  },
  [ExtensionPoint.RECOGNISER]: {
    label: "Recogniser",
    Icon: Fingerprint,
    means: "Proposes what a file is, for the Libraries that ask it. The Server decides.",
  },
  [ExtensionPoint.BLOB_STORE]: {
    label: "Blob store",
    Icon: Database,
    means: "Keeps the Server’s stored images, once an Admin picks it as the image store.",
  },
  [ExtensionPoint.AUTOMATION]: {
    label: "Automation",
    Icon: CalendarClock,
    means: "Runs Jobs on their schedules, and acts on the Server as far as its Permissions allow.",
  },
  [ExtensionPoint.HOME_ROW]: {
    label: "Home rows",
    Icon: Rows3,
    means: "Gives rows of titles for every User’s home page.",
  },
  [ExtensionPoint.SUBTITLE_SOURCE]: {
    label: "Subtitle source",
    Icon: Captions,
    means: "Searches an outside service for a Version’s subtitles and downloads one a User picks.",
  },
  [ExtensionPoint.SEGMENT_SOURCE]: {
    label: "Segment source",
    Icon: SkipForward,
    means: "Says where a Version’s Intro, Recap and Credits are, for the Libraries that ask it.",
  },
  [ExtensionPoint.SEARCH_SOURCE]: {
    label: "Search source",
    Icon: Search,
    means: "Adds titles to every User’s search, such as by what they are about.",
  },
  [ExtensionPoint.SIMILAR_SOURCE]: {
    label: "Similar source",
    Icon: Shapes,
    means: "Says which titles are like a title, for its Similar titles.",
  },
  [ExtensionPoint.SERVER_LINKER]: {
    label: "Server Link",
    Icon: Link,
    means: "Links the Server to your Account on the Hub, from Settings’ Server section.",
  },
};

/** What a point's failing calls leave undone, for the points where that isn't plain. */
export const failureMeans: Partial<Record<ExtensionPoint, string>> = {
  [ExtensionPoint.RECOGNISER]:
    "Until it answers, the Items it is asked about stay undecided and are tried again at the next Scan; the rest of Matching goes on.",
  [ExtensionPoint.HOME_ROW]:
    "Users go on seeing the rows it last gave, and a visit to home 15 minutes or more later asks it again.",
};
