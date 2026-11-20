import { fetchAll, fetchSubtitle } from "@/features/player/lib/fetch-subtitle";
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
  });
  // Its messaging never hears of a worker that won't load or crashes, so `ready` would never settle.
  // oxlint-disable-next-line no-underscore-dangle -- JASSUB's README offers its `_` fields for this
  renderer._worker.addEventListener("error", failed);
  const start = async () => {
    try {
      await renderer.ready;
      ready();
    } catch {
      failed();
    }
  };
  void start();
  return {
    dispose: () => {
      // It rethrows a failed start, which `failed` has already reported.
      void Promise.allSettled([renderer.destroy()]);
    },
  };
};
