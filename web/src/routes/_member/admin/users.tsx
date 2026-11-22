import { createFileRoute, redirect } from "@tanstack/react-router";

/**
 * Where the Users page was. Users is a section of Settings now, so a link here lands on Home with
 * it open. Outside `_admin`, whose check would refuse a page `adminPages` no longer lists.
 */
export const Route = createFileRoute("/_member/admin/users")({
  beforeLoad: () => {
    throw redirect({ to: "/", search: { settings: "users" }, replace: true });
  },
});
