import { createFileRoute, redirect } from "@tanstack/react-router";

/** Admin opens on the first page the User may open. The layout refuses a User who may open none. */
export const Route = createFileRoute("/_member/_admin/admin/")({
  beforeLoad: ({ context: { adminPages } }) => {
    throw redirect({ to: adminPages[0]?.to ?? "/" });
  },
});
