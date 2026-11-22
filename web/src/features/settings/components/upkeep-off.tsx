import { Link } from "@tanstack/react-router";
import { CircleOff } from "lucide-react";
import { buttonVariants } from "nama-ui/components/button";

import { UPKEEP } from "@/features/admin-backups/lib/back-up-job";
import { StatusCard } from "@/features/settings/components/status-card";

/** Says the Server writes no Backups while Upkeep, whose Job writes them, is disabled. */
export function UpkeepOff() {
  const enable = (
    <Link
      to="."
      search={(rest) => ({ ...rest, settings: "plugins", plugin: UPKEEP })}
      className={buttonVariants({ variant: "secondary" })}
    >
      Enable Upkeep
    </Link>
  );
  return (
    <StatusCard
      setting="schedule"
      Icon={CircleOff}
      title="The Server doesn’t back itself up"
      end={enable}
    >
      <span className="text-xs text-pretty text-muted-foreground">
        Upkeep, the Plugin that writes Backups, is disabled.
      </span>
    </StatusCard>
  );
}
