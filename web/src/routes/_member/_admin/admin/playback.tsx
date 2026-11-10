import { createQueryOptions, useSuspenseQuery } from "@connectrpc/connect-query";
import { createFileRoute } from "@tanstack/react-router";
import { CircleCheck, CircleSlash } from "lucide-react";

import { AdminList, AdminRow } from "@/components/admin-list";
import { Page } from "@/components/page";
import { SegmentFolder, playbackSettingsQuery } from "@/components/segment-folder";
import { StateLabel } from "@/components/state-label";
import { type FoundProgram, ServerService } from "@/gen/nama/v1/server_pb";

export const Route = createFileRoute("/_member/_admin/admin/playback")({
  loader: ({ context: { queryClient, transport } }) =>
    Promise.all([
      queryClient.query({
        ...createQueryOptions(ServerService.method.getPlaybackStatus, {}, { transport }),
        staleTime: "static",
      }),
      queryClient.query(playbackSettingsQuery(transport)),
    ]),
  component: Playback,
});

function Playback() {
  const { ffmpeg, ffprobe, ffmpegProblem, ffprobeProblem } = useSuspenseQuery(
    ServerService.method.getPlaybackStatus,
    {},
  ).data;
  return (
    <Page heading="Playback">
      <p className="mt-2 max-w-prose text-pretty text-muted-foreground">
        The Server Direct Plays a Version as it is. To Remux or Transcode one for a Device that
        can’t play it, it needs ffmpeg and ffprobe, which it looks for when it starts: at the paths
        set in its config file, or else on the PATH.
      </p>
      {(ffmpeg === undefined || ffprobe === undefined) && (
        <p className="mt-4 max-w-prose text-pretty">
          Without them it can only Direct Play. Install ffmpeg, which comes with ffprobe, or set
          their paths in the config file, then restart the Server.
        </p>
      )}
      <AdminList>
        <ProgramRow name="ffmpeg" found={ffmpeg} problem={ffmpegProblem} />
        <ProgramRow name="ffprobe" found={ffprobe} problem={ffprobeProblem} />
      </AdminList>
      <SegmentFolder />
    </Page>
  );
}

function ProgramRow({
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
