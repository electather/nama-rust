import { createFileRoute, redirect } from "@tanstack/react-router";

/** Where a Plugin's page was. Settings' Plugins section opens it in place of its list. */
export const Route = createFileRoute("/_member/admin/plugins/$pluginId")({
  beforeLoad: ({ params: { pluginId } }) => {
    throw redirect({ to: "/", search: { settings: "plugins", plugin: pluginId }, replace: true });
  },
});
