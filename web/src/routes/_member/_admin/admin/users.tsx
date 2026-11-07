import { createFileRoute } from "@tanstack/react-router";

import { Page } from "@/components/page";

// ponytail: a placeholder the chrome can link to, until the Users page lands (#68).
export const Route = createFileRoute("/_member/_admin/admin/users")({
  component: () => (
    <Page heading="Users">
      <p className="mt-2 text-muted-foreground">The Users on this Server will be here.</p>
    </Page>
  ),
});
