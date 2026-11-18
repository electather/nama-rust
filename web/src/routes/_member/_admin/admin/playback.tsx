import { createFileRoute } from "@tanstack/react-router";

import { Playback } from "@/features/admin-playback/components/playback";
import { playbackSettingsQuery } from "@/features/admin-playback/data/use-playback-settings";
import { playbackStatusQuery } from "@/features/admin-playback/data/use-playback-status";

export const Route = createFileRoute("/_member/_admin/admin/playback")({
  loader: ({ context: { queryClient, transport } }) =>
    Promise.all([
      queryClient.query(playbackStatusQuery(transport)),
      queryClient.query(playbackSettingsQuery(transport)),
    ]),
  component: Playback,
});
