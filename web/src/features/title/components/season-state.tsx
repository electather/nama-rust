import { Library } from "lucide-react";

import type { FoundSeason } from "@/gen/nama/v1/request_pb";
import { RequestStateLabel } from "@/shared/components/request-state/request-state-label";
import { StateLabel } from "@/shared/components/state-label";

/** Where a Season of a found Show is: in a Library, or its Request's state. */
export function SeasonState({ season }: { season: FoundSeason }) {
  if (season.inLibrary) {
    return <StateLabel label="In a Library" Icon={Library} />;
  }
  return season.request && <RequestStateLabel state={season.request.state} />;
}
