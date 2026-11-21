import { Slider as SliderPrimitive } from "@base-ui/react/slider";
import type { CSSProperties } from "react";

declare module "react" {
  interface CSSProperties {
    /** How far a section of a split slider track has played, as a width within it. */
    "--played"?: string;
    /** How far a section of a split slider track is loaded, as a width within it. */
    "--buffered"?: string;
    /** How far into a section of a split slider track the pointer lights it, as a width within it. */
    "--lit"?: string;
  }
}

/** Where a split track's fills reach, and the pointer, as fractions 0 to 1 of the whole track. */
interface Fills {
  value: number;
  buffered: number;
  point: number | undefined;
}

/** `at` as a CSS width within the section from `start` to `end`, all fractions of the track. */
function widthIn(at: number, start: number, end: number) {
  return `${Math.min(1, Math.max(0, (at - start) / (end - start))) * 100}%`;
}

function isPointed(point: number | undefined, start: number, end: number) {
  return point !== undefined && point >= start && point < end;
}

/** One section, as wide as it is long, which grows while the pointer is over it. */
function Section({ start, end, fills }: { start: number; end: number; fills: Fills }) {
  const style: CSSProperties = {
    "--grow": end - start,
    "--played": widthIn(fills.value, start, end),
    "--buffered": widthIn(fills.buffered, start, end),
    "--lit": widthIn(fills.point ?? 0, start, end),
  };
  return (
    <div
      data-pointed={isPointed(fills.point, start, end) || undefined}
      style={style}
      className="relative h-full shrink-0 grow-(--grow) basis-0 overflow-hidden rounded-full bg-foreground/25 transition-transform duration-150 ease-out data-pointed:scale-y-200 motion-reduce:transition-none"
    >
      <div className="absolute inset-y-0 inset-s-0 w-(--buffered) bg-foreground/25" />
      {/* Lit up to the pointer, as the whole track is. */}
      {fills.point !== undefined && (
        <div className="absolute inset-y-0 inset-s-0 w-(--lit) bg-foreground/25" />
      )}
      <div className="absolute inset-y-0 inset-s-0 w-(--played) bg-primary" />
    </div>
  );
}

/**
 * The track split into sections starting at `starts`, as the player's artboards draw Chapters,
 * with the gaps between them. `value` and `buffered` are in the slider's units, up to `max`.
 */
function SplitTrack({
  starts,
  value = 0,
  max = 100,
  buffered = 0,
  point,
}: {
  starts: number[];
  value: number | undefined;
  max: number | undefined;
  buffered: number | undefined;
  point: number | undefined;
}) {
  const fills = { value: value / max, buffered: buffered / max, point };
  return (
    <SliderPrimitive.Track
      data-slot="slider-track"
      className="relative flex h-1 grow gap-0.75 select-none"
    >
      {starts.map((start, index) => (
        <Section key={start} start={start} end={starts[index + 1] ?? 1} fills={fills} />
      ))}
    </SliderPrimitive.Track>
  );
}

/** The track as one: what's `buffered` of `max`, lit up to the pointer while `lit`, and the fill. */
function WholeTrack({
  buffered,
  max,
  lit,
}: {
  buffered: number | undefined;
  max: number | undefined;
  lit: boolean;
}) {
  return (
    <SliderPrimitive.Track
      data-slot="slider-track"
      className="relative h-1 grow overflow-hidden rounded-full bg-foreground/25 transition-all duration-200 ease-out select-none group-hover/slider:h-1.5 motion-reduce:transition-none"
    >
      {buffered !== undefined && (
        <progress
          aria-hidden
          value={buffered}
          max={max}
          className="absolute inset-0 size-full appearance-none bg-transparent [&::-moz-progress-bar]:bg-foreground/25 [&::-webkit-progress-bar]:bg-transparent [&::-webkit-progress-value]:bg-foreground/25"
        />
      )}
      {lit && (
        <div
          aria-hidden
          className="absolute inset-y-0 inset-s-0 w-(--point) bg-foreground/25 group-data-disabled/slider:hidden"
        />
      )}
      <SliderPrimitive.Indicator data-slot="slider-range" className="h-full bg-primary" />
    </SliderPrimitive.Track>
  );
}

/**
 * A slider's track: split at `sections`, fractions 0 to 1 starting at 0, when given, or else one
 * track that thickens under the pointer. `point` is how far along the pointer is, while it's over,
 * and `lit` says whether to light the track up to it.
 */
function SliderTrack({
  sections,
  value,
  max,
  buffered,
  point,
  lit,
}: {
  sections: number[] | undefined;
  value: number | undefined;
  max: number | undefined;
  buffered: number | undefined;
  point: number | undefined;
  lit: boolean;
}) {
  return sections ? (
    <SplitTrack starts={sections} value={value} max={max} buffered={buffered} point={point} />
  ) : (
    <WholeTrack buffered={buffered} max={max} lit={lit} />
  );
}

export { SliderTrack };
