import type { Device } from "@/gen/nama/v1/sign_in_pb";
import { dayOf } from "@/shared/lib/title-facts";

/** When `device` signed in and was last seen. */
export function DeviceTimes({ device }: { device: Device }) {
  return (
    <p className="text-sm text-muted-foreground">
      {/* Each phrase stays whole when the line wraps. */}
      <span className="whitespace-nowrap">Signed in {dayOf(device.createdAt)}</span> ·{" "}
      <span className="whitespace-nowrap">Last seen {dayOf(device.lastSeenAt)}</span>
    </p>
  );
}
