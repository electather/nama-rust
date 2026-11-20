import { fetchAll, fetchSubtitle } from "@/features/player/lib/fetch-subtitle";
import { drawnTimeOffset } from "@/features/player/lib/subtitle-offset";
import type { Render } from "@/features/player/lib/subtitle-render";

/** Draws an ASS subtitle with libass, with all the fonts its file carries. */
export const renderAss: Render = async ({ video, subtitle: { url, fontUrls }, ready, failed }) => {
  const [ass, fonts] = await Promise.all([fetchSubtitle(url), fetchAll(fontUrls)]);
  if (ass === null || fonts === null) {
    return null;
  }
  // Fetched only once an ASS subtitle is turned on.
  const { default: JASSUB } = await import("jassub");
  // It mounts its canvas beside the `<video>`, in the player, which full screen carries. It keeps
  // its own fallback font, and is ready only once it has loaded these.
  const renderer = new JASSUB({
    video,
    subContent: new TextDecoder().decode(ass),
    fonts: fonts.map((font) => new Uint8Array(font)),
    // Just the file's fonts and the fallback, never the Device's own (ADR 0033).
    queryFonts: false,
  });
  // oxlint-disable-next-line no-underscore-dangle -- JASSUB's README offers its `_` fields for this
  const worker = renderer._worker;
  // A crash can follow a failed start, and either can land after it's disposed of.
  let finished = false;
  const fail = () => {
    if (!finished) {
      finished = true;
      failed();
    }
  };
  // Its messaging never hears of a worker that won't load or crashes, so `ready` would never settle.
  worker.addEventListener("error", fail);
  const reportReady = async () => {
    try {
      await renderer.ready;
    } catch {
      fail();
      return;
    }
    if (!finished) {
      ready();
    }
  };
  void reportReady();
  return {
    dispose: () => {
      finished = true;
      // It rethrows a failed start, which `fail` has already reported.
      void Promise.allSettled([renderer.destroy()]);
      // `destroy` ends the worker only once `ready` settles, which a crashed worker's never does.
      worker.terminate();
    },
    shift: (offsetMs) => {
      renderer.timeOffset = drawnTimeOffset(offsetMs);
      // It draws the new time at the next frame, which a paused video never reaches: this redraws.
      void Promise.allSettled([renderer.resize()]);
    },
  };
};
