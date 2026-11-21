import { Link } from "@tanstack/react-router";

import { UPKEEP } from "@/features/admin-backups/lib/back-up-job";
import { textLink } from "@/shared/lib/text-link-classes";

/** Says the Server writes no Backups by itself while Upkeep, whose Job writes them, is disabled. */
export function UpkeepDisabled() {
  return (
    <p className="mt-6 max-w-prose text-pretty">
      Upkeep is disabled, so the Server doesn’t back itself up.{" "}
      <Link to="/admin/plugins/$pluginId" params={{ pluginId: UPKEEP }} className={textLink}>
        Enable Upkeep
      </Link>{" "}
      to back up every day, or now.
    </p>
  );
}
