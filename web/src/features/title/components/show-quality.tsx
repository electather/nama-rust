import type { FoundShow } from "@/gen/nama/v1/request_pb";
import { qualityName } from "@/shared/lib/quality-name";

/** The Quality `show`'s Seasons download at, once it has one. */
export function ShowQuality({ show }: { show: FoundShow }) {
  return (
    show.quality && (
      <p className="mt-3 text-sm text-muted-foreground">
        Every Season downloads at {qualityName(show.quality)}, the Show’s Quality.
      </p>
    )
  );
}
