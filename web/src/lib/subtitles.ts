import { type RefObject, useEffect, useEffectEvent, useState } from "react";

import { create } from "@bufbuild/protobuf";
import type { PgsRenderer } from "libbitsub";

import { type Track, TrackSchema } from "@/gen/nama/v1/browse_pb";
import { stopDrawingPgs } from "@/lib/device-decoding";

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
 * Draws the PGS subtitle at the URL `drawn` over the `<video>`, or none, and says how each drawn so
 * far is getting on: loading until it's fetched and read, or failed if it can't be fetched. When
 * the renderer won't start or throws, this Device stops drawing PGS and `onRendererFailed` runs.
 * A drawn subtitle stays where its Track puts it, under the controls.
 */
export function usePgsSubtitle(
  videoRef: RefObject<HTMLVideoElement | null>,
  drawn: string | undefined,
  onRendererFailed: () => void,
): Partial<Record<string, SubtitleState>> {
  const [states, setStates] = useState<Partial<Record<string, SubtitleState>>>({});
  const rendererFailed = useEffectEvent(() => {
    stopDrawingPgs();
    onRendererFailed();
  });
  useEffect(() => {
    const video = videoRef.current;
    const settle = (url: string) => (state: SubtitleState) => {
      setStates((before) => ({ ...before, [url]: state }));
    };
    return drawn !== undefined && video
      ? drawPgs(video, drawn, settle(drawn), rendererFailed)
      : undefined;
  }, [videoRef, drawn]);
  return states;
}

/**
 * Draws the PGS subtitle at `url` over `video`, until the cleanup it returns, saying how it's
 * getting on with `settle`, or calling `rendererFailed` if the renderer won't start or throws.
 */
function drawPgs(
  video: HTMLVideoElement,
  url: string,
  settle: (state: SubtitleState) => void,
  rendererFailed: () => void,
) {
  let renderer: PgsRenderer | undefined;
  let left = false;
  const failed = () => {
    if (!left) rendererFailed();
  };
  const start = async () => {
    const sup = await fetchSubtitle(url);
    if (left) return;
    if (sup === null) {
      settle("failed");
      return;
    }
    // Fetched only once a PGS subtitle is turned on.
    const { PgsRenderer: Renderer } = await import("libbitsub");
    if (left) return;
    // It mounts its canvas in the `<video>`'s parent, the player, which full screen carries.
    renderer = new Renderer({
      video,
      subContent: sup,
      onLoaded: () => {
        settle("ready");
      },
      // Not `onError`, which misses a renderer that won't start.
      onEvent: (event) => {
        if (event.type === "error") failed();
      },
    });
  };
  start().catch(failed);
  return () => {
    left = true;
    renderer?.dispose();
  };
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
