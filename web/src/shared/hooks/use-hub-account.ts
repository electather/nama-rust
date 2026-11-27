import { useRouteContext } from "@tanstack/react-router";

/**
 * The Account signed in at the Hub, as the layout over the page read it before it loaded, or
 * `null` at a Server or before one signs in.
 */
export function useHubAccount() {
  // Not strict: the Hub's top bar shows under `_signed-out` too, on a page a signed-in Account
  // opens there, such as a Sign-in Link's, so `_member` isn't the only route that reads it.
  return useRouteContext({ strict: false, select: ({ account }) => account ?? null });
}
