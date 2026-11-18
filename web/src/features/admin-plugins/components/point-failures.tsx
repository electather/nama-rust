import { PointFailing } from "@/features/admin-plugins/components/point-failing";
import type { ExtensionPointFailure } from "@/gen/nama/v1/plugin_pb";

/**
 * Each of the Plugin's Extension Points that keeps failing: how many calls in a row failed, when
 * the last one did and what it said. Nothing while its calls answer, and nothing for a point from a
 * newer Server than this app knows.
 */
export function PointFailures({ failures }: { failures: ExtensionPointFailure[] }) {
  return failures.map((failure) => <PointFailing key={failure.point} failure={failure} />);
}
