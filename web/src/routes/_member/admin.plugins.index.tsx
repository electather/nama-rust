import { createFileRoute, redirect } from "@tanstack/react-router";

/** Where the Plugins page was. Settings' Plugins section does what it did. */
export const Route = createFileRoute("/_member/admin/plugins/")({
  beforeLoad: () => {
    throw redirect({ to: "/", search: { settings: "plugins" }, replace: true });
  },
});
