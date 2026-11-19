import { createFileRoute, redirect } from "@tanstack/react-router";

import { Setup } from "@/features/auth/components/setup";
import { isSetupRequired } from "@/shared/hooks/access";

export const Route = createFileRoute("/_signed-out/setup")({
  beforeLoad: async ({ context }) => {
    if (!(await isSetupRequired(context))) {
      throw redirect({ to: "/" });
    }
  },
  component: Setup,
});
