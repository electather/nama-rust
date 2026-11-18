import { CircleCheck } from "lucide-react";

import { hardwareLabel } from "@/features/admin-playback/lib/hardware";
import { toneMappings } from "@/features/admin-playback/lib/hardware-wording";
import type { FoundHardware } from "@/gen/nama/v1/server_pb";
import { AdminRow } from "@/shared/components/admin-list/admin-row";
import { StateLabel } from "@/shared/components/state-label";

/**
 * A hardware acceleration API that passed its test encode, on its GPU if the Server names one.
 * Unless the Server `canTranscode`, tone-mapping and Automatic don't apply yet.
 */
export function HardwareRow({
  found,
  canTranscode,
}: {
  found: FoundHardware;
  canTranscode: boolean;
}) {
  return (
    <AdminRow end={<StateLabel label="Passed" Icon={CircleCheck} />}>
      <h3 className="font-medium">{hardwareLabel(found)}</h3>
      {canTranscode && (
        <p className="text-sm text-pretty text-muted-foreground">
          {toneMappings.get(found.toneMapping)}
          {found.automatic && " Automatic picks this one."}
        </p>
      )}
    </AdminRow>
  );
}
