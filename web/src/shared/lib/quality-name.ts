import type { RequestedQuality } from "@/gen/nama/v1/request_pb";

/** A Quality's name, or the service's own profile's, marked so, when no Quality offers it. */
export function qualityName({ id, name }: RequestedQuality) {
  return id === 0n ? `${name} (set in the service)` : name;
}
