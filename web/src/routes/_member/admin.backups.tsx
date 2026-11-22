import { createFileRoute, redirect } from "@tanstack/react-router";

/** Where the Backups page was. Backups is a section of Settings now, so a link here opens it. */
export const Route = createFileRoute("/_member/admin/backups")({
  beforeLoad: () => {
    throw redirect({ to: "/", search: { settings: "backups" }, replace: true });
  },
});
