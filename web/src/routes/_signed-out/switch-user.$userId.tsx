import { createFileRoute, redirect } from "@tanstack/react-router";

import { SwitchPinPrompt } from "@/features/auth/components/switch-pin-prompt";

export const Route = createFileRoute("/_signed-out/switch-user/$userId")({
  beforeLoad: ({ context, params, search }) => {
    const user = context.signedIn.deviceUsers.find((each) => String(each.id) === params.userId);
    if (!user) {
      throw redirect({ to: "/switch-user", search, replace: true });
    }
    return { user };
  },
  component: SwitchPinRoute,
});

function SwitchPinRoute() {
  const { redirect: back } = Route.useSearch();
  const { user } = Route.useRouteContext();
  return <SwitchPinPrompt user={user} redirect={back} />;
}
