import { Link } from "@tanstack/react-router";

import { whyNoSourceSearches } from "@/features/admin-libraries/lib/title-sources";
import type { TitleSource } from "@/gen/nama/v1/library_pb";
import { Permission } from "@/gen/nama/v1/sign_in_pb";
import { useMay } from "@/shared/hooks/use-may";
import { settingsDetailParams } from "@/shared/lib/settings-detail-params";
import { textLink } from "@/shared/lib/text-link-classes";

/**
 * Why the Library's Items wait to be Matched, if no source in `order` is running and searches:
 * each source in turn, and whether it isn't running or only adds artwork. Links to the Plugins
 * only for a User who may enable one there.
 */
export function NoSourceSearches({ order, listed }: { order: string[]; listed: TitleSource[] }) {
  const managesSettings = useMay(Permission.MANAGE_SETTINGS);
  const why = whyNoSourceSearches(order, listed);
  if (why === undefined) {
    return null;
  }
  const plugins = managesSettings ? (
    <Link
      to="."
      search={(rest) => ({ ...rest, ...settingsDetailParams, settings: "plugins" })}
      className={textLink}
    >
      Plugins
    </Link>
  ) : (
    "Plugins"
  );
  return (
    <p className="text-sm text-pretty">
      No source above can search for titles: {why}. The Library’s Items wait to be Matched until one
      can. Enable or set up a Plugin that searches under {plugins}, or add one here.
    </p>
  );
}
