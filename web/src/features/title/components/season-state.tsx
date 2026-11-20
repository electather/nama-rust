import { Library } from "lucide-react";

import type { FoundSeason } from "@/gen/nama/v1/request_pb";
import { RequestPill } from "@/shared/components/request-state/request-pill";
import { StateLabel } from "@/shared/components/state-label";

/** Where a Season of a found Show is: in a Library, or its Request's pill. */
export function SeasonState({ season }: { season: FoundSeason }) {
  if (season.inLibrary) {
    return <StateLabel label="In a Library" Icon={Library} />;
  }
  return season.request && <RequestPill request={season.request} />;
}
