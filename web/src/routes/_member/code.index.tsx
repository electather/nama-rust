import { createFileRoute, useLocation } from "@tanstack/react-router";

import { EnterCode } from "@/features/auth/components/enter-code";

export const Route = createFileRoute("/_member/code/")({
  // `?code=` is what a phone opens by scanning the QR code a signing-in Device shows.
  validateSearch: (search): { code?: string | undefined } => ({
    code: typeof search["code"] === "string" ? search["code"] : undefined,
  }),
  component: EnterCodeRoute,
});

function EnterCodeRoute() {
  const { code } = Route.useSearch();
  const refused = useLocation({ select: (location) => location.state.refusedCode });
  return <EnterCode scanned={code} refused={refused} />;
}
