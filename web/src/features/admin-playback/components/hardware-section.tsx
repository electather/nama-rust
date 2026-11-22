import { HardwareRow } from "@/features/admin-playback/components/hardware-row";
import { hardwareIntro } from "@/features/admin-playback/lib/hardware-wording";
import type { GetPlaybackStatusResponse } from "@/gen/nama/v1/server_pb";

/** The hardware acceleration that passed its test, and what Transcodes run on because of it. */
export function HardwareSection({ status }: { status: GetPlaybackStatusResponse }) {
  const { hardware, canTranscode } = status;
  return (
    <>
      <p className="max-w-prose text-sm text-pretty text-muted-foreground">
        {hardwareIntro(status)} The Server tests VAAPI on each GPU, Intel QSV on each Intel one, and
        NVIDIA NVENC and Apple VideoToolbox when it starts, so restart it after adding a GPU.
      </p>
      {hardware.length === 0 ? (
        <p className="mt-4 max-w-prose text-sm text-pretty">
          {canTranscode
            ? "None passed its test, so Transcodes run in software."
            : "None passed its test."}
        </p>
      ) : (
        <ul className="-mx-3 mt-4 flex flex-col">
          {hardware.map((found) => (
            <HardwareRow
              key={`${found.api}@${found.gpu?.id ?? ""}`}
              found={found}
              canTranscode={canTranscode}
            />
          ))}
        </ul>
      )}
    </>
  );
}
