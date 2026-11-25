import { UserRound } from "lucide-react";
import { Button } from "nama-ui/components/button";

import { ItemRow } from "@/features/settings/components/item-row";
import { useUnlinkLinkedUser } from "@/features/settings/data/use-unlink-linked-user";
import type { LinkedUser } from "@/gen/nama/hub/v1/link_pb";
import { IconTile } from "@/shared/components/icon-tile";
import { dayOf } from "@/shared/lib/title-facts";

/** One of the Account's Linked Users: who, on which Server, since when, and Unlinking them. */
export function LinkedUserItem({ user }: { user: LinkedUser }) {
  const unlink = useUnlinkLinkedUser();
  const name = `${user.name} on ${user.serverName}`;
  return (
    <ItemRow
      beside
      start={<IconTile Icon={UserRound} />}
      name={name}
      meta={`Linked ${dayOf(user.linkedAt)}`}
      failed={unlink.error ? "The Hub couldn’t Unlink them just now. Try again." : undefined}
    >
      <Button
        variant="ghost"
        aria-label={`Unlink ${name}`}
        disabled={unlink.isPending}
        onClick={() => {
          unlink.mutate({ id: user.id });
        }}
      >
        Unlink
      </Button>
    </ItemRow>
  );
}
