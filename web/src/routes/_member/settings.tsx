import { createFileRoute } from "@tanstack/react-router";

import { Page } from "@/components/page";

// ponytail: a placeholder the chrome can link to, until the Settings page lands (#68).
export const Route = createFileRoute("/_member/settings")({
  component: () => (
    <Page heading="Settings">
      <p className="mt-2 text-muted-foreground">Your password and Devices will be here.</p>
    </Page>
  ),
});
