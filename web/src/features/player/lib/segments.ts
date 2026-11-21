import { type Segment, SegmentKind } from "@/gen/nama/v1/playback_pb";

const SKIP_LABELS: Partial<Record<SegmentKind, string>> = {
  [SegmentKind.INTRO]: "Skip intro",
  [SegmentKind.RECAP]: "Skip recap",
};

/** The Intro or Recap playing at `time`, with its button's label and where it ends, both in seconds. */
export function skippable(
  segments: Segment[],
  { time, duration }: { time: number; duration: number },
) {
  // An unset end runs to the end of the Version.
  const endOf = ({ endMs }: Segment) => (endMs === undefined ? duration : Number(endMs) / 1000);
  const playing = segments.find(
    (segment) =>
      SKIP_LABELS[segment.kind] !== undefined &&
      time >= Number(segment.startMs) / 1000 &&
      time < endOf(segment),
  );
  return playing && { label: SKIP_LABELS[playing.kind] ?? "", end: endOf(playing) };
}

/** When the Credits start, in seconds, or `undefined` without a Credits Segment. */
export function creditsStart(segments: Segment[]) {
  const credits = segments.find(({ kind }) => kind === SegmentKind.CREDITS);
  return credits === undefined ? undefined : Number(credits.startMs) / 1000;
}
