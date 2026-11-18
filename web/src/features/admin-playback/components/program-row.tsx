import { CircleCheck, CircleSlash } from "lucide-react";

import type { FoundProgram } from "@/gen/nama/v1/server_pb";
import { AdminRow } from "@/shared/components/admin-list/admin-row";
import { StateLabel } from "@/shared/components/state-label";

/** ffmpeg or ffprobe: its version and path if the Server found it, or else why it can't use it. */
export function ProgramRow({
  name,
  found,
  problem,
}: {
  name: string;
  found: FoundProgram | undefined;
  problem: string;
}) {
  return (
    <AdminRow
      end={
        found === undefined ? (
          <StateLabel label="Can’t use" Icon={CircleSlash} />
        ) : (
          <StateLabel label="Found" Icon={CircleCheck} />
        )
      }
    >
      <h2 className="font-medium">{name}</h2>
      {found === undefined ? (
        <p className="text-sm text-pretty wrap-anywhere text-muted-foreground">{problem}</p>
      ) : (
        <>
          <p className="text-sm">version {found.version}</p>
          <p className="text-sm break-all text-muted-foreground">{found.path}</p>
        </>
      )}
    </AdminRow>
  );
}
