import { Link } from "@tanstack/react-router";
import { UserRound } from "lucide-react";

import { ItemRow } from "@/features/settings/components/item-row";
import type { User } from "@/gen/nama/v1/sign_in_pb";
import { IconTile } from "@/shared/components/icon-tile";
import { labelsOf } from "@/shared/lib/permissions";

/** One User on the list: their name over their Permissions. All of the row opens them. */
export function UserItem({ user, isYou }: { user: User; isYou: boolean }) {
  const name = (
    <Link
      to="."
      search={(rest) => ({ ...rest, user: Number(user.id) })}
      className="outline-none after:absolute after:inset-0 after:rounded-lg focus-visible:after:ring-3 focus-visible:after:ring-ring/50"
    >
      {user.name}
    </Link>
  );
  const about = isYou ? `You · ${labelsOf(user.permissions)}` : labelsOf(user.permissions);
  return <ItemRow linked start={<IconTile Icon={UserRound} />} name={name} meta={about} />;
}
