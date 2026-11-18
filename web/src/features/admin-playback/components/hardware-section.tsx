import { HardwareRow } from "@/features/admin-playback/components/hardware-row";
import { hardwareIntro } from "@/features/admin-playback/lib/hardware-wording";
import type { GetPlaybackStatusResponse } from "@/gen/nama/v1/server_pb";
import { AdminList } from "@/shared/components/admin-list/admin-list";

/** The hardware acceleration that passed its test, and what Transcodes run on because of it. */
export function HardwareSection({ status }: { status: GetPlaybackStatusResponse }) {
  const { hardware, canTranscode } = status;
  return (
    <section aria-labelledby="hardware" className="mt-12">
      <h2 id="hardware" className="font-medium">
        Hardware acceleration
      </h2>
      <p className="mt-2 max-w-prose text-pretty text-muted-foreground">
        {hardwareIntro(status)} The Server tests VAAPI on each GPU, Intel QSV on each Intel one, and
        NVIDIA NVENC and Apple VideoToolbox when it starts, so restart it after adding a GPU.
      </p>
      {hardware.length === 0 ? (
        <p className="mt-4 max-w-prose text-pretty">
          {canTranscode
            ? "None passed its test, so Transcodes run in software."
            : "None passed its test."}
        </p>
      ) : (
        <AdminList>
          {hardware.map((found) => (
            <HardwareRow
              key={`${found.api}@${found.gpu?.id ?? ""}`}
              found={found}
              canTranscode={canTranscode}
            />
          ))}
        </AdminList>
      )}
    </section>
  );
}
