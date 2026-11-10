import { createQueryOptions, useSuspenseQuery } from "@connectrpc/connect-query";
import { createFileRoute } from "@tanstack/react-router";
import { CircleCheck, CircleSlash } from "lucide-react";

import { Page } from "@/components/page";
import { StateLabel } from "@/components/state-label";
import { type FoundProgram, ServerService } from "@/gen/nama/v1/server_pb";

export const Route = createFileRoute("/_member/_admin/admin/playback")({
  loader: ({ context: { queryClient, transport } }) =>
    queryClient.query({
      ...createQueryOptions(ServerService.method.getPlaybackStatus, {}, { transport }),
      staleTime: "static",
    }),
  component: Playback,
});

function Playback() {
  const { ffmpeg, ffprobe } = useSuspenseQuery(ServerService.method.getPlaybackStatus, {}).data;
  return (
    <Page heading="Playback">
      <p className="mt-2 max-w-prose text-pretty text-muted-foreground">
        The Server Direct Plays a Version as it is. To Remux or Transcode one for a Device that
        can’t play it, it needs ffmpeg and ffprobe, which it looks for on the PATH when it starts.
      </p>
      {(ffmpeg === undefined || ffprobe === undefined) && (
        <p className="mt-4 max-w-prose text-pretty">
          Without them it can only Direct Play. Install ffmpeg, which comes with ffprobe, then
          restart the Server.
        </p>
      )}
      {/* Pulled out by the rows' own padding, so the names line up with the heading. */}
      <ul className="mt-8 -mx-3 flex max-w-2xl flex-col">
        <ProgramRow name="ffmpeg" found={ffmpeg} />
        <ProgramRow name="ffprobe" found={ffprobe} />
      </ul>
    </Page>
  );
}

function ProgramRow({ name, found }: { name: string; found: FoundProgram | undefined }) {
  return (
    <li className="flex flex-wrap items-center gap-x-3 gap-y-2 border-b px-3 py-3 last:border-b-0">
      <div className="flex min-w-0 flex-1 basis-64 flex-col items-start">
        <h2 className="font-medium">{name}</h2>
        {found === undefined ? (
          <p className="text-sm text-muted-foreground">{name} wasn’t found on the PATH.</p>
        ) : (
          <>
            <p className="text-sm">version {found.version}</p>
            <p className="text-sm break-all text-muted-foreground">{found.path}</p>
          </>
        )}
      </div>
      {found === undefined ? (
        <StateLabel label="Not found" Icon={CircleSlash} />
      ) : (
        <StateLabel label="Found" Icon={CircleCheck} />
      )}
    </li>
  );
}
