import { useSuspenseQuery } from "@connectrpc/connect-query";
import { createFileRoute } from "@tanstack/react-router";
import { CircleCheck, CircleSlash } from "lucide-react";

import { AdminList, AdminRow } from "@/components/admin-list";
import { Page } from "@/components/page";
import {
  hardwareName,
  PlaybackSettingsForm,
  playbackSettingsQuery,
  playbackStatusQuery,
} from "@/components/playback-settings";
import { StateLabel } from "@/components/state-label";
import { type FoundHardware, type FoundProgram, ServerService } from "@/gen/nama/v1/server_pb";

export const Route = createFileRoute("/_member/_admin/admin/playback")({
  loader: ({ context: { queryClient, transport } }) =>
    Promise.all([
      queryClient.query(playbackStatusQuery(transport)),
      queryClient.query(playbackSettingsQuery(transport)),
    ]),
  component: Playback,
});

function Playback() {
  const { ffmpeg, ffprobe, ffmpegProblem, ffprobeProblem, hardware } = useSuspenseQuery(
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
      <section aria-labelledby="hardware" className="mt-12">
        <h2 id="hardware" className="font-medium">
          Hardware acceleration
        </h2>
        <p className="mt-2 max-w-prose text-pretty text-muted-foreground">
          What Transcodes can run on besides software. The Server tests VAAPI, Intel QSV, NVIDIA
          NVENC and Apple VideoToolbox when it starts, so restart it after adding a GPU.
        </p>
        {hardware.length === 0 ? (
          <p className="mt-4 max-w-prose text-pretty">
            None passed its test, so Transcodes run in software.
          </p>
        ) : (
          <AdminList>
            {hardware.map((found, index) => (
              <HardwareRow key={found.api} found={found} automatic={index === 0} />
            ))}
          </AdminList>
        )}
      </section>
      <PlaybackSettingsForm />
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

/** A hardware acceleration API that passed its test encode, `automatic` if Automatic picks it. */
function HardwareRow({ found, automatic }: { found: FoundHardware; automatic: boolean }) {
  return (
    <AdminRow end={<StateLabel label="Passed" Icon={CircleCheck} />}>
      <h3 className="font-medium">{hardwareName(found.api)}</h3>
      <p className="text-sm text-pretty text-muted-foreground">
        {found.toneMaps
          ? "Tone-maps HDR to SDR on the GPU too."
          : "Tone-maps HDR to SDR in software, between decoding and encoding on the GPU."}
        {automatic && " Automatic picks this one."}
      </p>
    </AdminRow>
  );
}
