import { HardDrive } from "lucide-react";

import { ItemRow } from "@/features/settings/components/item-row";
import type { LinkedServer } from "@/gen/nama/hub/v1/link_pb";
import { IconTile } from "@/shared/components/icon-tile";
import { dayOf } from "@/shared/lib/title-facts";

/** One of the Account's Servers, opening its own web app at its Server URL if its Admin set one. */
export function LinkedServerItem({ server }: { server: LinkedServer }) {
  // ponytail: a link out until the Hub shows each Server's content itself (ADR 0105).
  const opens = server.url !== "";
  const name = opens ? (
    <a
      href={server.url}
      className="outline-none after:absolute after:inset-0 after:rounded-lg focus-visible:after:ring-3 focus-visible:after:ring-ring/50"
    >
      {server.name}
    </a>
  ) : (
    server.name
  );
  return (
    <ItemRow
      linked={opens}
      start={<IconTile Icon={HardDrive} />}
      name={name}
      meta={`Linked ${dayOf(server.linkedAt)}`}
    />
  );
}
