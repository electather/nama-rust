import { createFileRoute, notFound } from "@tanstack/react-router";

import { isAtHub } from "@/shared/hooks/access";

/**
 * Ways to sign in only a Server has: the Hub answers them "Page not found" (ADR 0105). They stay
 * under `/sign-in` for its `redirect`.
 */
export const Route = createFileRoute("/_signed-out/sign-in/_server")({
  beforeLoad: async ({ context }) => {
    if (await isAtHub(context)) {
      throw notFound();
    }
  },
});
