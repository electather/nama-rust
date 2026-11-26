import { createContext, useContext } from "react";

import { useAskForSeasons } from "@/features/title/data/use-ask-for-seasons";
import type { FoundShow } from "@/gen/nama/v1/request_pb";
import { useSignedInUser } from "@/shared/hooks/use-signed-in-user";

/**
 * Asking for Seasons of `show` from anywhere on its page, its main button, a missing Episode's
 * corner or its "…", which share `asking`, so the page says once why one was refused. `you` is the
 * signed-in User.
 */
export interface SeasonAsking {
  show: FoundShow;
  you: bigint | undefined;
  asking: ReturnType<typeof useAskForSeasons>;
}

/** The page's `SeasonAsking`, unset where the Show wasn't found or the User may not Request it. */
export const SeasonAskingContext = createContext<SeasonAsking | undefined>(undefined);

/** The page's `SeasonAsking`, if its Seasons may be asked for. */
export function useSeasonAsking() {
  return useContext(SeasonAskingContext);
}

/** The `SeasonAsking` a page of `show` provides, nothing until it's found. */
export function usePageSeasonAsking(show: FoundShow | undefined): SeasonAsking | undefined {
  const asking = useAskForSeasons();
  const you = useSignedInUser()?.id;
  return show && { show, you, asking };
}

/** Asks for `seasons` of the page's Show, at the User's Quality or `quality`. */
export function askFor({ show, asking }: SeasonAsking, seasonNumbers: number[], quality?: bigint) {
  asking.mutate({ showId: show.id, seasonNumbers, quality });
}
