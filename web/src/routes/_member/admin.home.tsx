import { createFileRoute, redirect } from "@tanstack/react-router";

/** Where the default Home page was. It's a section of Settings now, so a link here opens that. */
export const Route = createFileRoute("/_member/admin/home")({
  beforeLoad: () => {
    throw redirect({ to: "/", search: { settings: "default-home" }, replace: true });
  },
});
