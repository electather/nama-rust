import { createFileRoute } from "@tanstack/react-router";

import { Libraries } from "@/features/admin-libraries/components/libraries";
import { librariesQuery } from "@/shared/hooks/use-libraries";

export const Route = createFileRoute("/_member/_admin/admin/libraries/")({
  loader: ({ context: { queryClient, transport } }) => queryClient.query(librariesQuery(transport)),
  component: Libraries,
});
