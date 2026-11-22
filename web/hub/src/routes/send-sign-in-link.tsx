import { SendSignInLinkPage } from "@hub/features/account/components/send-sign-in-link-page";
import { createFileRoute } from "@tanstack/react-router";

/** Asking for a Sign-in Link. `?continue=` names the Hub path the sign-in continues at. */
export const Route = createFileRoute("/send-sign-in-link")({
  validateSearch: (search): { continue?: string | undefined } => ({
    continue: typeof search["continue"] === "string" ? search["continue"] : undefined,
  }),
  component: SendSignInLinkRoute,
});

function SendSignInLinkRoute() {
  const { continue: continueTo } = Route.useSearch();
  return <SendSignInLinkPage continueTo={continueTo ?? "/"} />;
}
