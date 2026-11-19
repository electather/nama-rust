import { createFileRoute, redirect } from "@tanstack/react-router";

import { CodeWaiting } from "@/features/auth/components/code-waiting";

export const Route = createFileRoute("/_signed-out/sign-in/code/waiting")({
  beforeLoad: ({ location, search }) => {
    const shown = location.state.waitingCode;
    if (shown === undefined) {
      throw redirect({ to: "/sign-in/code", search, replace: true });
    }
    return { shown };
  },
  component: CodeWaitingRoute,
});

function CodeWaitingRoute() {
  const { redirect: back } = Route.useSearch();
  const { shown } = Route.useRouteContext();
  return <CodeWaiting shown={shown} redirect={back} />;
}
