import { createFileRoute, redirect } from "@tanstack/react-router";

import { firstSection } from "@/features/settings/lib/sections";

/**
 * Where the Settings page was. Settings is a modal over a page now (`src/app/settings-loader.tsx`),
 * so a link here lands on Home with it open.
 */
export const Route = createFileRoute("/_member/settings")({
  beforeLoad: () => {
    throw redirect({ to: "/", search: { settings: firstSection }, replace: true });
  },
});
