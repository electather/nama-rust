import { createFileRoute } from "@tanstack/react-router";

import { Jobs } from "@/features/admin-jobs/components/jobs";
import { jobsQuery } from "@/features/admin-jobs/data/use-jobs";

export const Route = createFileRoute("/_member/_admin/admin/jobs")({
  loader: ({ context: { queryClient, transport } }) => queryClient.query(jobsQuery(transport)),
  component: Jobs,
});
