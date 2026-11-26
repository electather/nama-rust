import { cornerClasses } from "@/features/title/lib/corner-classes";
import type { RequestState } from "@/gen/nama/v1/request_pb";
import { shownOnTitle } from "@/shared/components/request-state/request-states";

/** A missing Episode's icon for its Season's Request `state`, such as a clock while Pending. */
export function StateCorner({ state }: { state: RequestState }) {
  const { label, Icon } = shownOnTitle(state);
  return (
    <span className={cornerClasses}>
      <Icon aria-hidden className="size-4.5" />
      <span className="sr-only">{label}</span>
    </span>
  );
}
