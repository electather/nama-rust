import { type RefObject, useEffect, useState } from "react";

import { create } from "@bufbuild/protobuf";

import { type Track, TrackSchema } from "@/gen/nama/v1/browse_pb";

/** How a subtitle turned on is getting on: being read out of the file, shown, or not to be had. */
export type SubtitleState = "loading" | "ready" | "failed";

/**
 * Which of the `<video>`'s `<track>`s shows, by URL, and how each turned on so far is getting on.
 * Turning one on or off only changes which shows, never the plan or what the `<video>` plays.
 */
export function useSubtitles(videoRef: RefObject<HTMLVideoElement | null>) {
  const [shown, setShown] = useState<string>();
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
  /** Shows the `<track>` at `url`, or none for `undefined`. */
  const show = (url: string | undefined) => {
    setShown(url);
    if (url !== undefined) setStates((before) => ({ [url]: "loading", ...before }));
  };
  return { shown, states, show };
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
