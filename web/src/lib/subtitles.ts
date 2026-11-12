import { type RefObject, useEffect, useEffectEvent, useState } from "react";

import { create } from "@bufbuild/protobuf";
import { useMutation } from "@connectrpc/connect-query";
// Only the WebAssembly worker: its asm.js twin, for a Device without WebAssembly, stays out.
import fallbackFont from "@jellyfin/libass-wasm/dist/js/default.woff2?url";
import assWorkerScript from "@jellyfin/libass-wasm/dist/js/subtitles-octopus-worker.js?url";
import assWasm from "@jellyfin/libass-wasm/dist/js/subtitles-octopus-worker.wasm?url";

import { type Track, TrackSchema } from "@/gen/nama/v1/browse_pb";
import {
  type PlaybackSubtitle,
  PickedTrackKind,
  PlaybackService,
  SubtitleFormat,
} from "@/gen/nama/v1/playback_pb";
import { retryIfUnreachable } from "@/lib/connect-errors";
import { stopDrawing } from "@/lib/device-decoding";

/** How a subtitle turned on is getting on: being read out of the file, shown, or not to be had. */
export type SubtitleState = "loading" | "ready" | "failed";

/**
 * Shows the `<video>`'s `<track>` at the URL `shown`, or none, and says how each shown so far is
 * getting on. Showing one only changes which shows, never the plan or what the `<video>` plays.
 */
export function useSubtitles(
  videoRef: RefObject<HTMLVideoElement | null>,
  shown: string | undefined,
): Partial<Record<string, SubtitleState>> {
  const [states, setStates] = useState<Partial<Record<string, SubtitleState>>>({});
  useEffect(() => {
    // A `<track>`'s `default` counts only when the `<video>` first loads, so its mode is set.
    for (const element of videoRef.current?.querySelectorAll("track") ?? []) {
      element.track.mode = element.getAttribute("src") === shown ? "showing" : "disabled";
    }
  }, [videoRef, shown]);
  useEffect(() => {
    const video = videoRef.current;
    // A `<track>`'s load and error don't bubble, so the `<video>` catches them on the way down.
    const settle =
      (state: SubtitleState) =>
      ({ target }: Event) => {
        if (!(target instanceof HTMLTrackElement)) return;
        clearOfControls(target.track);
        setStates((before) => ({ ...before, [target.getAttribute("src") ?? ""]: state }));
      };
    const [onLoad, onError] = [settle("ready"), settle("failed")];
    video?.addEventListener("load", onLoad, true);
    video?.addEventListener("error", onError, true);
    return () => {
      video?.removeEventListener("load", onLoad, true);
      video?.removeEventListener("error", onError, true);
    };
  }, [videoRef]);
  // Shown and not yet settled is loading.
  return shown === undefined ? states : { [shown]: "loading", ...states };
}

/**
 * Draws the plan's subtitle `drawn`, one in a format this Device draws, over the `<video>`, or
 * none, and says how each drawn so far is getting on: loading until it and the fonts it's drawn
 * with are fetched and read, or failed if they can't be fetched. When the renderer won't start or
 * throws, this Device stops drawing that format and `onRendererFailed` runs. A drawn subtitle
 * stays where its Track puts it, under the controls.
 */
export function useDrawnSubtitle(
  videoRef: RefObject<HTMLVideoElement | null>,
  drawn: PlaybackSubtitle | undefined,
  onRendererFailed: () => void,
): Partial<Record<string, SubtitleState>> {
  const [states, setStates] = useState<Partial<Record<string, SubtitleState>>>({});
  const rendererFailed = useEffectEvent((format: SubtitleFormat) => {
    stopDrawing(format);
    onRendererFailed();
  });
  useEffect(() => {
    const video = videoRef.current;
    const start = (subtitle: PlaybackSubtitle, on: HTMLVideoElement) => {
      const settle = (state: SubtitleState) => {
        setStates((before) => ({ ...before, [subtitle.url]: state }));
      };
      const render = subtitle.format === SubtitleFormat.ASS ? renderAss : renderBitmap;
      return draw(on, subtitle, render, settle, () => {
        rendererFailed(subtitle.format);
      });
    };
    return drawn !== undefined && video ? start(drawn, video) : undefined;
  }, [videoRef, drawn]);
  return states;
}

/** A renderer drawing a subtitle over a `<video>` until it's disposed of. */
interface Renderer {
  dispose: () => void;
}

/**
 * Makes a renderer drawing `subtitle` over `video`, calling `ready` once it draws, or `failed` if
 * it won't start or throws. `null` when what it draws can't be fetched.
 */
type Render = (
  video: HTMLVideoElement,
  subtitle: PlaybackSubtitle,
  ready: () => void,
  failed: () => void,
) => Promise<Renderer | null>;

/**
 * Draws `subtitle` over `video` with what `render` makes, until the cleanup it returns, saying how
 * it's getting on with `settle`, or calling `rendererFailed` if the renderer won't start or throws.
 */
function draw(
  video: HTMLVideoElement,
  subtitle: PlaybackSubtitle,
  render: Render,
  settle: (state: SubtitleState) => void,
  rendererFailed: () => void,
) {
  let renderer: Renderer | undefined;
  let left = false;
  const failed = () => {
    if (!left) rendererFailed();
  };
  const ready = () => {
    settle("ready");
  };
  const start = async () => {
    // Fetched afresh each time it's turned on, so what an earlier one came to no longer says.
    settle("loading");
    const made = await render(video, subtitle, ready, failed);
    if (made === null) {
      if (!left) settle("failed");
    } else if (left) {
      made.dispose();
    } else {
      renderer = made;
    }
  };
  start().catch(failed);
  return () => {
    left = true;
    renderer?.dispose();
  };
}

/** Draws a PGS subtitle, or a VobSub one from its Track copied into Matroska, with libbitsub. */
const renderBitmap: Render = async (video, { url, format }, ready, failed) => {
  const content = await fetchSubtitle(url);
  if (content === null) return null;
  // Fetched only once such a subtitle is turned on.
  const libbitsub = await import("libbitsub");
  // It mounts its canvas in the `<video>`'s parent, the player, which full screen carries.
  const options = {
    video,
    subContent: content,
    onLoaded: ready,
    // Not `onError`, which misses a renderer that won't start.
    onEvent: (event: { type: string }) => {
      if (event.type === "error") failed();
    },
  };
  if (format !== SubtitleFormat.VOBSUB) return new libbitsub.PgsRenderer(options);
  // ponytail: libbitsub 1.13.2 never reports a VobSub load that throws (altqx/libbitsub#14), so
  // the picker stays on "Loading…"; a release that fixes it lets the fall back to Burn-in catch it.
  // The name says Matroska, which carries the palette and size, rather than a `.sub` and `.idx`.
  return new libbitsub.VobSubRenderer({ ...options, fileName: "track.mks" });
};

/** Draws an ASS subtitle with libass, with all the fonts its file carries. */
const renderAss: Render = async (video, { url, fontUrls }, ready, failed) => {
  const [ass, fonts] = await Promise.all([fetchSubtitle(url), fetchAll(fontUrls)]);
  if (ass === null || fonts === null) return null;
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
      if (!disposedItself) renderer.dispose();
      for (const blob of [worker, ...fetched]) URL.revokeObjectURL(blob);
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

/** Each of `urls`, or `null` if any can't be fetched. */
async function fetchAll(urls: string[]) {
  const fetched = await Promise.all(urls.map((url) => fetchSubtitle(url)));
  return fetched.every((body) => body !== null) ? fetched : null;
}

/** The subtitle at `url`, or `null` if it can't be fetched. */
async function fetchSubtitle(url: string) {
  try {
    const response = await fetch(url);
    return response.ok ? await response.arrayBuffer() : null;
  } catch {
    return null;
  }
}

/**
 * Ends the cues the file doesn't place 85% of the way down the video rather than at its bottom,
 * where the controls over the bottom of the video would cover them.
 */
function clearOfControls({ cues }: TextTrack) {
  const unplaced = Array.from(cues ?? []).filter(
    (cue): cue is VTTCue => cue instanceof VTTCue && cue.line === "auto",
  );
  for (const cue of unplaced) {
    Object.assign(cue, { snapToLines: false, line: 85, lineAlign: "end" });
  }
}

const languageNames = new Intl.DisplayNames(["en"], { type: "language" });

/**
 * A subtitle or audio Track's name, its language or else its title, and what else tells it apart:
 * its title, Forced and SDH.
 */
export function trackLabel(track: Track | undefined) {
  const facts = track ?? create(TrackSchema);
  const language = languageName(facts.language);
  return {
    name: language ?? facts.title ?? "Unknown language",
    details: details(facts, language !== undefined),
  };
}

/** Its title, unless that's its name, then Forced and SDH. */
function details({ title, forced, sdh }: Track, named: boolean) {
  return [named && title, forced && "Forced", sdh && "SDH"]
    .filter((detail) => typeof detail === "string")
    .join(" · ");
}

/** The language a file tags as `code`, such as "English" for `eng`, if it tags one. */
function languageName(code: string | undefined) {
  // ISO 639-2's `und`, undetermined, would be named "root".
  return code === undefined || code === "und" ? undefined : nameOf(code);
}

function nameOf(code: string) {
  try {
    return languageNames.of(code);
  } catch {
    // A tag that isn't a language code at all is shown as it is.
    return code;
  }
}

/**
 * Reports the Track of a kind the User picked while playing Version `versionId`, by its place in
 * the Version's Tracks, or `undefined` for subtitles off. Reports go out in the background, in
 * order, and a failed one is sent again, never shown.
 */
export function usePickReporter(versionId: bigint) {
  const { mutate } = useMutation(PlaybackService.method.reportTrackPick, {
    retry: retryIfUnreachable,
    // One at a time, so a retried pick never lands after a newer one.
    scope: { id: "report-track-pick" },
  });
  return (kind: PickedTrackKind, track: number | undefined) => {
    mutate({ versionId, kind, track });
  };
}
