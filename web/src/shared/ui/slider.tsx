import { Slider as SliderPrimitive } from "@base-ui/react/slider";
import { cn } from "cn";
import { type CSSProperties, type PointerEvent, type ReactNode, useState } from "react";

import { SliderTrack } from "@/shared/ui/slider-track";

/** How far along `element` from its start edge, 0 to 1, the pointer at `clientX` is. */
function fractionAt(element: HTMLElement, clientX: number) {
  const { left, width } = element.getBoundingClientRect();
  const fromLeft = Math.min(1, Math.max(0, (clientX - left) / width));
  return getComputedStyle(element).direction === "rtl" ? 1 - fromLeft : fromLeft;
}

/** Where along a slider the pointer is: how far, 0 to 1, and how wide the slider is, in pixels. */
interface Pointed {
  fraction: number;
  width: number;
}

/**
 * Where along an element the pointer is, for a slider to show what's there: `at` while the pointer
 * is over it, and `--point` as a percentage from its start edge. Spread the handlers and style on
 * the element.
 */
function usePointedAt() {
  const [at, setAt] = useState<Pointed>();
  const style: CSSProperties & Record<"--point", string> = {
    "--point": `${(at?.fraction ?? 0) * 100}%`,
  };
  return {
    at,
    style,
    onPointerMove: ({ currentTarget, clientX }: PointerEvent<HTMLElement>) => {
      setAt({
        fraction: fractionAt(currentTarget, clientX),
        width: currentTarget.getBoundingClientRect().width,
      });
    },
    onPointerLeave: () => {
      setAt(undefined);
    },
  };
}

// The player's artboards: a track of Ink at 25%, which holds up over video, a Near-white fill and thumb, and the focus ring
// at full strength, since the slider sits over video. A disabled thumb is Quiet ink, never faded,
// as DESIGN.md says of a disabled button. Restyled from shadcn's to one value, so its thumb can
// carry the label. As YouTube's seek bar does, the track thickens and the thumb grows under the
// pointer (on a plain ease-out: on the exponential one a 2px change is over in a frame), and with
// `renderPoint` it lights the track up to the pointer and shows what's there.
function Slider({
  className,
  label,
  valueText,
  buffered,
  renderPoint,
  sections,
  ...props
}: SliderPrimitive.Root.Props<number> & {
  /** Names the slider for assistive technology. */
  label: string;
  /** The value in words, such as "4:05 of 1:42:10". */
  valueText?: string;
  /** How far along the track is loaded, in the slider's units, drawn under the fill. */
  buffered?: number;
  /**
   * What's at the pointer, given where along the track it is, drawn over the track: placed from
   * the track's start edge, such as with `inset-s-*`, and above it, with `bottom-full`.
   */
  renderPoint?: (at: Pointed) => ReactNode;
  /** Where each section of the track starts, as fractions 0 to 1, the first 0, to draw them apart. */
  sections?: number[] | undefined;
}) {
  const { at, ...pointer } = usePointedAt();
  const card = at && renderPoint?.(at);
  // Lit only where something is shown at the pointer, as the seek bar's card; not the volume.
  const lit = Boolean(card);
  return (
    <SliderPrimitive.Root
      className={cn("w-full", className)}
      data-slot="slider"
      thumbAlignment="edge"
      {...props}
    >
      <SliderPrimitive.Control
        className="group/slider relative flex h-6 w-full cursor-pointer touch-none items-center select-none data-disabled:cursor-default"
        {...pointer}
      >
        <SliderTrack
          sections={sections}
          value={props.value}
          max={props.max}
          buffered={buffered}
          point={at?.fraction}
          lit={lit}
        />
        {/* Focus lands on the thumb's hidden range input, so the ring follows `has-focus-visible`. */}
        <SliderPrimitive.Thumb
          data-slot="slider-thumb"
          aria-label={label}
          aria-valuetext={valueText}
          className="relative block size-3.5 shrink-0 scale-75 rounded-full bg-primary ring-ring/50 transition duration-150 ease-out-expo select-none group-hover/slider:scale-100 after:absolute after:-inset-2 hover:ring-3 has-focus-visible:scale-100 has-focus-visible:ring-3 has-focus-visible:ring-ring data-dragging:scale-100 motion-reduce:transition-none data-disabled:pointer-events-none data-disabled:bg-muted-foreground"
        />
        {/* Pointer-only: the thumb's `aria-valuetext` already says where it is. */}
        <div aria-hidden className="pointer-events-none group-data-disabled/slider:hidden">
          {card}
        </div>
      </SliderPrimitive.Control>
    </SliderPrimitive.Root>
  );
}

export { Slider, type Pointed };
