import { fetchSubtitle } from "@/features/player/lib/fetch-subtitle";
import type { Render } from "@/features/player/lib/subtitle-render";
import { SubtitleFormat } from "@/gen/nama/v1/playback_pb";

/** Draws a PGS subtitle, or a VobSub one from its Track copied into Matroska, with libbitsub. */
export const renderBitmap: Render = async ({ video, subtitle: { url, format }, ready, failed }) => {
  const content = await fetchSubtitle(url);
  if (content === null) {
    return null;
  }
  // Fetched only once such a subtitle is turned on.
  const libbitsub = await import("libbitsub");
  // It mounts its canvas in the `<video>`'s parent, the player, which full screen carries.
  const options = {
    video,
    subContent: content,
    onLoaded: ready,
    // Not `onError`, which misses a renderer that won't start.
    onEvent: (event: { type: string }) => {
      if (event.type === "error") {
        failed();
      }
    },
  };
  if (format !== SubtitleFormat.VOBSUB) {
    return new libbitsub.PgsRenderer(options);
  }
  // ponytail: libbitsub 1.13.2 never reports a VobSub load that throws (altqx/libbitsub#14), so
  // the picker stays on "Loading…"; a release that fixes it lets the fallback to Burn-in catch it.
  // The name says Matroska, which carries the palette and size, rather than a `.sub` and `.idx`.
  return new libbitsub.VobSubRenderer({ ...options, fileName: "track.mks" });
};
