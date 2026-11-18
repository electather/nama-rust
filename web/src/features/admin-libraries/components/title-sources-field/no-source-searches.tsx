import { Link, useRouteContext } from "@tanstack/react-router";

import { whyNoSourceSearches } from "@/features/admin-libraries/lib/title-sources";
import type { TitleSource } from "@/gen/nama/v1/library_pb";
import { Permission } from "@/gen/nama/v1/sign_in_pb";
import { may } from "@/shared/lib/permissions";

/**
 * Why the Library's Items wait to be Matched, if no source in `order` is running and searches:
 * each source in turn, and whether it isn't running or only adds artwork. Links to the Plugins
 * only for a User who may enable one there.
 */
export function NoSourceSearches({ order, listed }: { order: string[]; listed: TitleSource[] }) {
  const { signedIn } = useRouteContext({ from: "/_member" });
  const why = whyNoSourceSearches(order, listed);
  if (why === undefined) {
    return null;
  }
  const plugins = may(signedIn.user, Permission.MANAGE_SETTINGS) ? (
    <Link
      to="/admin/plugins"
      className="rounded-sm underline decoration-muted-foreground underline-offset-4 outline-none hover:decoration-foreground focus-visible:ring-3 focus-visible:ring-ring/50"
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
