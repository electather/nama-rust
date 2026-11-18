import { Link } from "@tanstack/react-router";

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
        to="/admin/plugins/$pluginId"
        params={{ pluginId }}
        className="rounded-sm text-sm underline decoration-muted-foreground underline-offset-4 outline-none hover:decoration-foreground focus-visible:ring-3 focus-visible:ring-ring/50"
      >
        Fix it in the Plugin’s settings
      </Link>
    )
  );
}
