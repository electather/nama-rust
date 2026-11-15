// What both the Plugins list and a Plugin's page show of it, and how its page changes its logo, kept
// in one module so each page stays within oxlint's `import/max-dependencies`.
import { createConnectQueryKey } from "@connectrpc/connect-query";
import { Activity, Bell, Film, KeyRound, Link, Puzzle, ScanSearch, Server, Tv } from "lucide-react";

import { ChangeImage, type ImageRules } from "@/components/edit-images";
import type { Shown } from "@/components/state-label";
import {
  ExtensionPoint,
  PluginPermission,
  PluginService,
  type PluginSummary,
} from "@/gen/nama/v1/plugin_pb";
import { pluginName } from "@/lib/plugins";

/** Each Extension Point in plain words, with the icon the web picks for it. */
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
};

/**
 * The Plugin Permissions an Admin granted the Plugin, if any, as a line of a wrapping flex row.
 * One from a newer Server than this app knows is left out.
 */
export function GrantedPermissions({ granted }: { granted: PluginPermission[] }) {
  const labels = granted.flatMap((permission) => pluginPermissions[permission]?.label ?? []);
  if (labels.length === 0) return null;
  return (
    <p className="w-full text-sm text-pretty text-muted-foreground">Granted: {labels.join(", ")}</p>
  );
}

/**
 * A Plugin's logo on a neutral tile, or a generic puzzle piece when it has none. Only ever an image
 * from its URL (`web/AGENTS.md`): as an image, SVG runs no script and loads nothing.
 */
export function PluginLogo({ plugin }: { plugin: { id: string; name: string; logoUrl: string } }) {
  const label = `${pluginName(plugin)} logo`;
  return (
    <div className="flex size-10 shrink-0 items-center justify-center overflow-hidden rounded-lg bg-muted">
      {plugin.logoUrl === "" ? (
        // oxlint-disable-next-line jsx-a11y/prefer-tag-over-role -- our own icon, inline so it follows the colour scheme, which an <img> can't
        <Puzzle role="img" aria-label={label} className="size-5 text-muted-foreground" />
      ) : (
        <img src={plugin.logoUrl} alt={label} className="size-full object-contain" />
      )}
    </div>
  );
}

/** A Plugin's logo may also be an SVG, and the Plugin queries send its URL. */
const pluginLogos: ImageRules = {
  accept: "image/jpeg,image/png,image/webp,image/svg+xml",
  refusals: {
    403: "This needs the Manage Settings Permission.",
    413: "That file is too big: an SVG can be up to 64 KB, other images up to 20 MB.",
    415: "That isn’t a JPEG, PNG, WebP or SVG image.",
    422: "That image is over 12 megapixels. Pick a smaller one.",
  },
  queryKey: createConnectQueryKey({ schema: PluginService, cardinality: undefined }),
};

/** Where an admin uploads their own logo for the Plugin, shown in place of its own until removed. */
export function LogoChoice({ summary }: { summary: PluginSummary }) {
  const image = {
    name: "Logo",
    custom: summary.customLogo,
    path: `/custom-images/plugin/${encodeURIComponent(summary.id)}/logo`,
  };
  return (
    <section aria-labelledby="logo" className="mt-10">
      <h2 id="logo" className="font-medium">
        Logo
      </h2>
      <p className="mt-1 max-w-prose text-sm text-pretty text-muted-foreground">
        {summary.customLogo
          ? "Everyone sees the logo you uploaded, until you remove it."
          : "Upload your own to show in place of the Plugin’s. JPEG, PNG or WebP up to 20 MB, or SVG up to 64 KB."}
      </p>
      <ChangeImage image={image} rules={pluginLogos} />
    </section>
  );
}
