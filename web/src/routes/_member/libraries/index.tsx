import { createFileRoute, redirect } from "@tanstack/react-router";

// Home lists every Library, so a Library's address trimmed to `/libraries` lands there.
export const Route = createFileRoute("/_member/libraries/")({
  beforeLoad: () => {
    throw redirect({ to: "/", replace: true });
  },
});
