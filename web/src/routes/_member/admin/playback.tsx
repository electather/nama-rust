import { createFileRoute, redirect } from "@tanstack/react-router";

/**
 * Where the Playback page was. It's the Server Playback section of Settings now, so a link here
 * lands on Home with it open. Settings checks the Permission, so this isn't an Admin page.
 */
export const Route = createFileRoute("/_member/admin/playback")({
  beforeLoad: () => {
    throw redirect({ to: "/", search: { settings: "server-playback" }, replace: true });
  },
});
