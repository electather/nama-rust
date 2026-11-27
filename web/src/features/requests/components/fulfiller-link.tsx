import { Link } from "@tanstack/react-router";
import { cn } from "cn";

import { detailSearch } from "@/shared/lib/settings-address";
import { textLink } from "@/shared/lib/text-link-classes";

/**
 * The link to the Plugin `pluginId`, whose settings fix why its Fulfiller didn't take a Request,
 * shown only to a User who `mayFix` them there: one with Manage settings.
 */
export function FulfillerLink({
  pluginId,
  mayFix,
}: {
  pluginId: string | undefined;
  mayFix: boolean;
}) {
  return (
    pluginId !== undefined &&
    mayFix && (
      <Link
        to="."
        search={(rest) => ({ ...rest, ...detailSearch({ kind: "plugin", id: pluginId }) })}
        className={cn(textLink, "text-sm")}
      >
        Fix it in the Plugin’s settings
      </Link>
    )
  );
}
