import { createFileRoute, notFound } from "@tanstack/react-router";

import { isAtHub } from "@/shared/hooks/access";

/** Signed-out pages only the Hub has: a Server answers them "Page not found" (ADR 0105). */
export const Route = createFileRoute("/_signed-out/_hub")({
  beforeLoad: async ({ context }) => {
    if (!(await isAtHub(context))) {
      throw notFound();
    }
  },
});
