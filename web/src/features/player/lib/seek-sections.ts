import type { FrameSheets } from "@/features/player/lib/preview-frame";
import { type Chapter, type Segment, SegmentKind } from "@/gen/nama/v1/playback_pb";

const SEGMENT_NAMES: Partial<Record<SegmentKind, string>> = {
  [SegmentKind.RECAP]: "Recap",
  [SegmentKind.INTRO]: "Intro",
  [SegmentKind.CREDITS]: "Credits",
};

/**
 * How near two edges may be before the later-ranked one is dropped, as a file's Chapters and a
 * Segment source's times for the same Intro rarely agree to the second.
 */
// ponytail: a fixed 5 s; weigh it against the sections' lengths if short ones get swallowed.
const NEAR_SECONDS = 5;

/** A section of the seek bar: where it starts, in seconds, and what it's called, if anything. */
export interface SeekSection {
  start: number;
  name: string | undefined;
}

/** What the seek bar splits at and shows under the pointer. */
export interface SeekBar {
  sections: SeekSection[];
  /** The Version's Preview frames, if it has any. */
  sheets: FrameSheets | undefined;
}

/** A named stretch, in seconds. */
interface Span {
  start: number;
  end: number;
  name: string;
}

function segmentSpans(segments: Segment[], duration: number): Span[] {
  return segments.flatMap(({ kind, startMs, endMs }) => {
    const name = SEGMENT_NAMES[kind];
    // An unset end runs to the end of the Version.
    const end = endMs === undefined ? duration : Number(endMs) / 1000;
    return name === undefined ? [] : [{ start: Number(startMs) / 1000, end, name }];
  });
}

/** Each Chapter, named by its title, or else "Chapter N"; none for fewer than two. */
function chapterSpans(chapters: Chapter[], duration: number): Span[] {
  if (chapters.length < 2) {
    return [];
  }
  const starts = chapters.map(({ startMs }) => Number(startMs) / 1000);
  return chapters.map(({ title }, index) => ({
    start: starts[index] ?? 0,
    end: starts[index + 1] ?? duration,
    name: title === "" ? `Chapter ${index + 1}` : title,
  }));
}

/**
 * Where the sections start: 0, then the edges of `spans` in order, each dropped when it's near
 * one already kept or near the end. Segments come first in `spans`, so their edges win.
 */
function edgesOf(spans: Span[], duration: number) {
  const kept = [0];
  for (const edge of spans.flatMap(({ start, end }) => [start, end])) {
    const far = kept.every((each) => Math.abs(each - edge) > NEAR_SECONDS);
    if (far && edge < duration - NEAR_SECONDS) {
      kept.push(edge);
    }
  }
  // oxlint-disable-next-line unicorn/no-array-sort -- sorts the array just built, and the lib target has no toSorted
  return kept.sort((a, b) => a - b);
}

function nameAt(spans: Span[], seconds: number) {
  return spans.find(({ start, end }) => start <= seconds && seconds < end)?.name;
}

/**
 * The seek bar's sections: each Recap, Intro and Credits on its own, and the file's Chapters
 * between them. None when that makes fewer than two, which keeps a plain seek bar.
 */
export function seekSections(chapters: Chapter[], segments: Segment[], duration: number) {
  const spans = [...segmentSpans(segments, duration), ...chapterSpans(chapters, duration)];
  const edges = duration > 0 ? edgesOf(spans, duration) : [];
  const sections: SeekSection[] = edges.map((start, index) => ({
    start,
    name: nameAt(spans, (start + (edges[index + 1] ?? duration)) / 2),
  }));
  return sections.length < 2 ? [] : sections;
}

/** The name of the section of `sections`, which ascend, at `seconds`, if it has one. */
export function sectionAt(sections: SeekSection[], seconds: number) {
  let name: string | undefined;
  // The lib target has no findLast: the last section to start by `seconds` holds it.
  for (const { start, name: named } of sections) {
    if (start <= seconds) {
      name = named;
    }
  }
  return name;
}

/**
 * Where the section before the one playing at `time` starts, for Ctrl/⌥+Left; the first section's
 * start while the first plays. `undefined` without sections.
 */
export function previousStart(sections: SeekSection[], time: number) {
  const playing = sections.filter(({ start }) => start <= time).length - 1;
  return sections[Math.max(0, playing - 1)]?.start;
}

/** Where the section after the one playing at `time` starts, for Ctrl/⌥+Right, if there is one. */
export function nextStart(sections: SeekSection[], time: number) {
  return sections.find(({ start }) => start > time)?.start;
}
