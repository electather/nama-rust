import { Link } from "@tanstack/react-router";
import { UserRound, UserRoundX } from "lucide-react";

import { ItemRow } from "@/features/settings/components/item-row";
import type { User } from "@/gen/nama/v1/sign_in_pb";
import { IconTile } from "@/shared/components/icon-tile";
import { labelsOf } from "@/shared/lib/permissions";

/**
 * One User on the list: their name over their Permissions, after whether they can't sign in. All
 * of the row opens them.
 */
export function UserItem({
  user,
  isYou,
  cantSignIn,
}: {
  user: User;
  isYou: boolean;
  cantSignIn: boolean;
}) {
  const name = (
    <Link
      to="."
      search={(rest) => ({ ...rest, user: Number(user.id) })}
      className="outline-none after:absolute after:inset-0 after:rounded-lg focus-visible:after:ring-3 focus-visible:after:ring-ring/50"
    >
      {user.name}
    </Link>
  );
  const about = [
    isYou && "You",
    cantSignIn && "Can’t sign in until given a password",
    labelsOf(user.permissions),
  ].filter((words) => words !== false);
  const icon = <IconTile Icon={cantSignIn ? UserRoundX : UserRound} />;
  return <ItemRow linked start={icon} name={name} meta={about.join(" · ")} />;
}
