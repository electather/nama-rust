import type { Backup } from "@/gen/nama/v1/server_pb";

const inUnit = (unit: "megabyte" | "gigabyte") =>
  new Intl.NumberFormat(undefined, { style: "unit", unit, maximumFractionDigits: 1 });
const megabytes = inUnit("megabyte");
const gigabytes = inUnit("gigabyte");

/** A Backup's size, in megabytes under a gigabyte, since a new Server's are that small. */
export function backupSize({ sizeBytes }: Backup) {
  const bytes = Number(sizeBytes);
  return bytes < 1e9 ? megabytes.format(bytes / 1e6) : gigabytes.format(bytes / 1e9);
}
