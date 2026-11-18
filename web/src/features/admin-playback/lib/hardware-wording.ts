import { type GetPlaybackStatusResponse, ToneMapping } from "@/gen/nama/v1/server_pb";

/** The line that introduces the hardware acceleration that passed its test. */
export function hardwareIntro(status: GetPlaybackStatusResponse) {
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

/** What tone-maps HDR to SDR in a Transcode on a hardware row, in plain words. */
export const toneMappings = new Map<ToneMapping, string>([
  [ToneMapping.GPU, "Tone-maps HDR to SDR on the GPU too."],
  [
    ToneMapping.SOFTWARE,
    "Tone-maps HDR to SDR in software, between decoding and encoding on the GPU.",
  ],
  [ToneMapping.NONE, "Can’t tone-map HDR to SDR, on the GPU or in software."],
]);
