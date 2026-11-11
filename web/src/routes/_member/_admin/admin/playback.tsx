import { useSuspenseQuery } from "@connectrpc/connect-query";
import { createFileRoute } from "@tanstack/react-router";
import { CircleCheck, CircleSlash } from "lucide-react";

import { AdminList, AdminRow } from "@/components/admin-list";
import { Page } from "@/components/page";
import {
  hardwareLabel,
  PlaybackSettingsForm,
  playbackSettingsQuery,
  playbackStatusQuery,
} from "@/components/playback-settings";
import { StateLabel } from "@/components/state-label";
import {
  type FoundHardware,
  type FoundProgram,
  type GetPlaybackStatusResponse,
  ServerService,
  ToneMapping,
} from "@/gen/nama/v1/server_pb";

export const Route = createFileRoute("/_member/_admin/admin/playback")({
  loader: ({ context: { queryClient, transport } }) =>
    Promise.all([
      queryClient.query(playbackStatusQuery(transport)),
      queryClient.query(playbackSettingsQuery(transport)),
    ]),
  component: Playback,
});

function Playback() {
  const status = useSuspenseQuery(ServerService.method.getPlaybackStatus, {}).data;
  const { ffmpeg, ffprobe, ffmpegProblem, ffprobeProblem } = status;
  return (
    <Page heading="Playback">
      <p className="mt-2 max-w-prose text-pretty text-muted-foreground">
        The Server Direct Plays a Version as it is. To Remux or Transcode one for a Device that
        can’t play it, it needs ffmpeg and ffprobe, which it looks for when it starts: at the paths
        set in its config file, or else on the PATH.
      </p>
      {ffmpeg === undefined || ffprobe === undefined ? (
        <p className="mt-4 max-w-prose text-pretty">
          Without them it can only Direct Play. Install ffmpeg, which comes with ffprobe, or set
          their paths in the config file, then restart the Server.
        </p>
      ) : (
        <MissingFromFfmpeg status={status} />
      )}
      <AdminList>
        <ProgramRow name="ffmpeg" found={ffmpeg} problem={ffmpegProblem} />
        <ProgramRow name="ffprobe" found={ffprobe} problem={ffprobeProblem} />
      </AdminList>
      <HardwareSection status={status} />
      <PlaybackSettingsForm />
    </Page>
  );
}

/** What the Server's ffmpeg lacks that a Transcode needs, if anything. */
function MissingFromFfmpeg({ status }: { status: GetPlaybackStatusResponse }) {
  if (!status.canTranscode && !status.hasLibx264) {
    return (
      <p className="mt-4 max-w-prose text-pretty">
        This Server’s ffmpeg lacks libx264, so it can’t Transcode, only Direct Play and Remux.
        Install an ffmpeg built with libx264, such as jellyfin-ffmpeg, which the Docker image comes
        with, then restart the Server.
      </p>
    );
  }
  if (!status.hasZscale) {
    return (
      <p className="mt-4 max-w-prose text-pretty">
        This Server’s ffmpeg lacks zscale, so a Transcode can’t tone-map HDR to SDR in software, and
        HDR looks washed out on an SDR screen unless the GPU tone-maps it. Install an ffmpeg built
        with zscale, such as jellyfin-ffmpeg, then restart the Server.
      </p>
    );
  }
  return null;
}

function HardwareSection({ status }: { status: GetPlaybackStatusResponse }) {
  const { hardware, canTranscode } = status;
  return (
    <section aria-labelledby="hardware" className="mt-12">
      <h2 id="hardware" className="font-medium">
        Hardware acceleration
      </h2>
      <p className="mt-2 max-w-prose text-pretty text-muted-foreground">
        {hardwareIntro(status)} The Server tests VAAPI on each GPU, Intel QSV on each Intel one, and
        NVIDIA NVENC and Apple VideoToolbox when it starts, so restart it after adding a GPU.
      </p>
      {hardware.length === 0 ? (
        <p className="mt-4 max-w-prose text-pretty">
          {canTranscode
            ? "None passed its test, so Transcodes run in software."
            : "None passed its test."}
        </p>
      ) : (
        <AdminList>
          {hardware.map((found) => (
            <HardwareRow
              key={`${found.api}@${found.gpu?.id ?? ""}`}
              found={found}
              canTranscode={canTranscode}
            />
          ))}
        </AdminList>
      )}
    </section>
  );
}

function hardwareIntro(status: GetPlaybackStatusResponse) {
  return status.hardware.length === 0 || status.canTranscode
    ? "What Transcodes can run on besides software."
    : whyNoTranscodeRunsOnHardware(status);
}

function whyNoTranscodeRunsOnHardware({ ffprobe, hasLibx264 }: GetPlaybackStatusResponse) {
  if (ffprobe === undefined) {
    return hasLibx264
      ? "The hardware below passed its test, but no Transcode runs on it until the Server finds ffprobe."
      : "The hardware below passed its test, but no Transcode runs on it until the Server finds ffprobe and its ffmpeg has libx264.";
  }
  return hasLibx264
    ? "The hardware below passed its test, but no Transcode runs on it."
    : "The hardware below passed its test, but no Transcode runs on it, or in software, until the Server’s ffmpeg has libx264.";
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

/**
 * A hardware acceleration API that passed its test encode, on its GPU if the Server names one.
 * Unless the Server `canTranscode`, tone-mapping and Automatic don't apply yet.
 */
function HardwareRow({ found, canTranscode }: { found: FoundHardware; canTranscode: boolean }) {
  return (
    <AdminRow end={<StateLabel label="Passed" Icon={CircleCheck} />}>
      <h3 className="font-medium">{hardwareLabel(found)}</h3>
      {canTranscode && (
        <p className="text-sm text-pretty text-muted-foreground">
          {toneMappings.get(found.toneMapping)}
          {found.automatic && " Automatic picks this one."}
        </p>
      )}
    </AdminRow>
  );
}

/** What tone-maps HDR to SDR in a Transcode on a hardware row, in plain words. */
const toneMappings = new Map<ToneMapping, string>([
  [ToneMapping.GPU, "Tone-maps HDR to SDR on the GPU too."],
  [
    ToneMapping.SOFTWARE,
    "Tone-maps HDR to SDR in software, between decoding and encoding on the GPU.",
  ],
  [ToneMapping.NONE, "Can’t tone-map HDR to SDR, on the GPU or in software."],
]);
