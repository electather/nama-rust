import { createFileRoute } from "@tanstack/react-router";

import { HomePage } from "@/features/home/components/home-page";
import { continueWatchingQuery } from "@/features/home/data/use-continue-watching";
import { librariesQuery } from "@/shared/hooks/use-libraries";

export const Route = createFileRoute("/_member/")({
  loader: ({ context: { queryClient, transport } }) =>
    Promise.all([
      queryClient.query(librariesQuery(transport)),
      queryClient.query(continueWatchingQuery(transport)),
    ]),
  component: HomePage,
});
