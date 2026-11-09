import { Activity, Bell, Film, KeyRound, Link, ScanSearch, Server, Tv } from "lucide-react";

import type { Shown } from "@/components/state-label";
import { ExtensionPoint } from "@/gen/nama/v1/plugin_pb";

/** Each Extension Point in the Glossary's words, with the icon the web picks for it. */
const points: Partial<Record<ExtensionPoint, Shown>> = {
  [ExtensionPoint.MOVIE_FULFILLER]: { label: "Fulfiller for Movies", Icon: Film },
  [ExtensionPoint.SHOW_FULFILLER]: { label: "Fulfiller for Shows", Icon: Tv },
  [ExtensionPoint.NOTIFIER]: { label: "Notifier", Icon: Bell },
  [ExtensionPoint.CONNECTOR]: { label: "Connector", Icon: Link },
  [ExtensionPoint.AUTHENTICATOR]: { label: "Authenticator", Icon: KeyRound },
  [ExtensionPoint.METADATA_PROVIDER]: { label: "Metadata Provider", Icon: ScanSearch },
  [ExtensionPoint.EVENTS_OBSERVER]: { label: "Observes Events", Icon: Activity },
  [ExtensionPoint.SERVER_STARTED_OBSERVER]: { label: "Observes Server start", Icon: Server },
};

/**
 * The Extension Points a Plugin takes part at, each an icon with its label below it. One from a
 * newer Server than this app knows is left out.
 */
export function ExtensionPoints({ extensionPoints }: { extensionPoints: ExtensionPoint[] }) {
  const shown = extensionPoints.flatMap((point) => points[point] ?? []);
  if (shown.length === 0) return null;
  return (
    <ul aria-label="Extension Points" className="mt-3 flex flex-wrap gap-x-5 gap-y-3">
      {shown.map(({ label, Icon }) => (
        <li key={label} className="flex flex-col items-start gap-1 text-sm text-muted-foreground">
          <Icon aria-hidden className="size-5" />
          {label}
        </li>
      ))}
    </ul>
  );
}
