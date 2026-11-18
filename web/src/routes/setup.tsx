import { createFileRoute, redirect } from "@tanstack/react-router";

import { Setup } from "@/features/auth/components/setup";
import { isSetupRequired } from "@/lib/access";

export const Route = createFileRoute("/setup")({
  beforeLoad: async ({ context }) => {
    if (!(await isSetupRequired(context))) {
      throw redirect({ to: "/" });
    }
  },
  component: Setup,
});
