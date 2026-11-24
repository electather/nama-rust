import type { Backup } from "@/gen/nama/v1/server_pb";
import { sizeOf } from "@/shared/lib/title-facts";

/** A Backup's size, read the way every other size in the app reads. */
export function backupSize({ sizeBytes }: Backup) {
  return sizeOf(sizeBytes);
}
