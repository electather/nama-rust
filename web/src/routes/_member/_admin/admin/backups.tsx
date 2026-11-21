import { createFileRoute } from "@tanstack/react-router";

import { Backups } from "@/features/admin-backups/components/backups";
import { backupsQuery } from "@/features/admin-backups/data/use-backups";
import { jobsQuery } from "@/features/admin-jobs/data/use-jobs";

export const Route = createFileRoute("/_member/_admin/admin/backups")({
  loader: ({ context: { queryClient, transport } }) =>
    Promise.all([
      queryClient.query(backupsQuery(transport)),
      queryClient.query(jobsQuery(transport)),
    ]),
  component: Backups,
});
