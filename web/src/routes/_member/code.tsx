import { createFileRoute, Outlet } from "@tanstack/react-router";

import { CodeLayout } from "@/features/auth/components/code-layout";

export const Route = createFileRoute("/_member/code")({
  component: CodeLayoutRoute,
});

function CodeLayoutRoute() {
  return (
    <CodeLayout>
      <Outlet />
    </CodeLayout>
  );
}
