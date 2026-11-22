import { Bell } from "lucide-react";

import { ConnectionActions } from "@/features/settings/components/connection-actions";
import { ItemRow } from "@/features/settings/components/item-row";
import { useConnectionChanges } from "@/features/settings/hooks/use-connection-changes";
import { about, pluginName } from "@/features/settings/lib/connection-wording";
import type { PluginConnection } from "@/gen/nama/v1/user_pb";
import { IconTile } from "@/shared/components/icon-tile";

/** One Plugin's connection: where it is, and the buttons to connect, cancel or disconnect. */
export function ConnectionItem({ connection }: { connection: PluginConnection }) {
  const name = pluginName(connection.pluginId);
  const changes = useConnectionChanges(name);
  return (
    <ItemRow
      start={<IconTile Icon={Bell} />}
      name={name}
      meta={about(connection.state, name)}
      failed={changes.failed}
    >
      <ConnectionActions connection={connection} name={name} changes={changes} />
    </ItemRow>
  );
}
