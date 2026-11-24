import { createFileRoute } from "@tanstack/react-router";

import { HomePage } from "@/features/home/components/home-page";
import { HomePending } from "@/features/home/components/home-pending";
import { continueWatchingQuery } from "@/features/home/data/use-continue-watching";
import { featuredQuery } from "@/features/home/data/use-featured";
import { homeRowsQuery } from "@/features/home/data/use-home-rows";
import { librariesQuery } from "@/shared/hooks/use-libraries";

/** A failed section shows its failure itself, when it reads its query, so the loader drops it. */
const shownBySection = () => {};

export const Route = createFileRoute("/_member/")({
  loader: async ({ context: { queryClient, transport } }) => {
    // The top of the page starts beside the layout rather than after it, and Home waits only on
    // its rows and Libraries, so neither shifts what's below once it shows: each section shows its
    // own skeleton until its titles arrive.
    queryClient.query(featuredQuery(transport)).catch(shownBySection);
    queryClient.infiniteQuery(continueWatchingQuery(transport)).catch(shownBySection);
    await Promise.all([
      queryClient.query(homeRowsQuery(transport)),
      queryClient.query(librariesQuery(transport)).catch(shownBySection),
    ]);
  },
  pendingComponent: HomePending,
  component: HomePage,
});
