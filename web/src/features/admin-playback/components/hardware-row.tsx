import { CircleCheck } from "lucide-react";

import { toneMappings } from "@/features/admin-playback/lib/hardware-wording";
import type { FoundHardware } from "@/gen/nama/v1/server_pb";
import { AdminRow } from "@/shared/components/admin-list/admin-row";
import { StateLabel } from "@/shared/components/state-label";
import { hardwareLabel } from "@/shared/lib/playback-names";

/**
 * A hardware acceleration API that passed its test encode, on its GPU if the Server names one.
 * Unless the Server `canTranscode`, tone-mapping, HEVC and Automatic don't apply yet.
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
      <h5 className="text-sm font-semibold">{hardwareLabel(found)}</h5>
      {canTranscode && (
        <p className="text-xs text-pretty text-muted-foreground">
          {toneMappings.get(found.toneMapping)}
          {!found.hevc && " Didn’t pass its test of HEVC, so Transcodes on it make H.264."}
          {found.automatic && " Automatic picks this one."}
        </p>
      )}
    </AdminRow>
  );
}
