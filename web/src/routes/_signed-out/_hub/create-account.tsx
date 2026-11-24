import { createFileRoute } from "@tanstack/react-router";

import { CreateAccount } from "@/features/auth/components/create-account";

export const Route = createFileRoute("/_signed-out/_hub/create-account")({
  component: CreateAccount,
});
