import { Library } from "lucide-react";

import type { FoundSeason } from "@/gen/nama/v1/request_pb";
import { RequestStatus } from "@/shared/components/request-state/request-status";
import { StateLabel } from "@/shared/components/state-label";
import { seasonName } from "@/shared/lib/title-facts";

/** Where a Season of a found Show is: in a Library, or its Request's status. */
export function SeasonState({ season }: { season: FoundSeason }) {
  if (season.inLibrary) {
    return <StateLabel label="In a Library" Icon={Library} />;
  }
  return (
    season.request && (
      <RequestStatus request={season.request} what={seasonName(season.seasonNumber)} />
    )
  );
}
