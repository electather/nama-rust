import { createFileRoute, redirect } from "@tanstack/react-router";

/** Admin opens on the Libraries. */
export const Route = createFileRoute("/_member/_admin/admin/")({
  beforeLoad: () => {
    throw redirect({ to: "/admin/libraries" });
  },
});
