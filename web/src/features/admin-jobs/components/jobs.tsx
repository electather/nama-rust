import { useRef, useState } from "react";

import { JobSheet } from "@/features/admin-jobs/components/job-sheet";
import { PluginJobs } from "@/features/admin-jobs/components/plugin-jobs";
import { useJobs } from "@/features/admin-jobs/data/use-jobs";
import { byPlugin } from "@/features/admin-jobs/lib/by-plugin";
import type { Job } from "@/gen/nama/v1/job_pb";
import { Page } from "@/shared/components/page";

/** The Jobs page: the enabled Plugins' Jobs, by Plugin, each opening a sheet of its Runs. */
export function Jobs() {
  const jobs = useJobs();
  const sheet = useRef<HTMLDialogElement>(null);
  const [open, setOpen] = useState<Job>();
  const show = (job: Job) => {
    setOpen(job);
    sheet.current?.showModal();
  };
  return (
    <Page heading="Jobs">
      <p className="mt-2 max-w-prose text-pretty text-muted-foreground">
        {jobs.length === 0
          ? "No enabled Plugin has Jobs."
          : "Work the enabled Plugins do on a schedule, such as tidying what the Server keeps."}
      </p>
      {byPlugin(jobs).map((group) => (
        <PluginJobs key={group.pluginId} group={group} onOpen={show} />
      ))}
      <JobSheet
        ref={sheet}
        job={open}
        onClose={() => {
          setOpen(undefined);
        }}
      />
    </Page>
  );
}
