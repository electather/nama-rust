import { createFileRoute } from "@tanstack/react-router";

import { NowPlaying } from "@/features/admin-now-playing/components/now-playing";
import { playsQuery } from "@/features/admin-now-playing/data/use-plays";
import { usersQuery } from "@/features/admin-users/data/use-users";

export const Route = createFileRoute("/_member/_admin/admin/now-playing")({
  loader: ({ context: { queryClient, transport } }) =>
    Promise.all([
      queryClient.query(playsQuery(transport)),
      queryClient.query(usersQuery(transport)),
    ]),
  component: NowPlaying,
});
