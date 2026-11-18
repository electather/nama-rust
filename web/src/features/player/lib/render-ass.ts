// Only the WebAssembly worker: its asm.js twin, for a Device without WebAssembly, stays out.
// fallow-ignore-next-line policy-violation:feature-layout/dynamic-loading -- `?url` gives the file's address, not its code, so nothing downloads until the renderer fetches it
import fallbackFont from "@jellyfin/libass-wasm/dist/js/default.woff2?url";
// fallow-ignore-next-line policy-violation:feature-layout/dynamic-loading -- `?url` gives the file's address, not its code, so nothing downloads until the renderer fetches it
import assWorkerScript from "@jellyfin/libass-wasm/dist/js/subtitles-octopus-worker.js?url";
// fallow-ignore-next-line policy-violation:feature-layout/dynamic-loading -- `?url` gives the file's address, not its code, so nothing downloads until the renderer fetches it
import assWasm from "@jellyfin/libass-wasm/dist/js/subtitles-octopus-worker.wasm?url";

import { fetchAll, fetchSubtitle } from "@/features/player/lib/fetch-subtitle";
import type { Render } from "@/features/player/lib/subtitle-render";

/** Draws an ASS subtitle with libass, with all the fonts its file carries. */
export const renderAss: Render = async ({ video, subtitle: { url, fontUrls }, ready, failed }) => {
  const [ass, fonts] = await Promise.all([fetchSubtitle(url), fetchAll(fontUrls)]);
  if (ass === null || fonts === null) {
    return null;
  }
  // Fetched only once an ASS subtitle is turned on.
  const { default: SubtitlesOctopus } = await import("@jellyfin/libass-wasm");
  // Handed over as fetched, so "Loading…" lasts until they all are.
  const fetched = fonts.map((font) => URL.createObjectURL(new Blob([font])));
  const worker = assWorker();
  let disposedItself = false;
  // It mounts its canvas beside the `<video>`, in the player, which full screen carries.
  const renderer = new SubtitlesOctopus({
    video,
    subContent: new TextDecoder().decode(ass),
    fonts: fetched,
    fallbackFont: absolute(fallbackFont),
    workerUrl: worker,
    onReady: ready,
    onError: () => {
      // It disposes of itself after an error, and throws if disposed of again.
      disposedItself = true;
      failed();
    },
  });
  return {
    dispose: () => {
      if (!disposedItself) {
        renderer.dispose();
      }
      for (const blob of [worker, ...fetched]) {
        URL.revokeObjectURL(blob);
      }
    },
  };
};

/**
 * A worker running libass. It looks for its WebAssembly beside itself under the name the package
 * gives it, which the build hashes, so it's told where the build put it.
 */
function assWorker() {
  const script = `self.Module = { locateFile: () => ${JSON.stringify(absolute(assWasm))} };
importScripts(${JSON.stringify(absolute(assWorkerScript))});`;
  return URL.createObjectURL(new Blob([script], { type: "text/javascript" }));
}

/** `url` whole, since a worker started from a blob can't resolve a relative one. */
function absolute(url: string) {
  return new URL(url, document.baseURI).href;
}
