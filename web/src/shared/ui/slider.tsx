import { Slider as SliderPrimitive } from "@base-ui/react/slider";
import { cn } from "cn";
import { type CSSProperties, type PointerEvent, useState } from "react";

/** How far along `element` from its start edge, 0 to 1, the pointer at `clientX` is. */
function fractionAt(element: HTMLElement, clientX: number) {
  const { left, width } = element.getBoundingClientRect();
  const fromLeft = Math.min(1, Math.max(0, (clientX - left) / width));
  return getComputedStyle(element).direction === "rtl" ? 1 - fromLeft : fromLeft;
}

/**
 * Where along an element the pointer is, for a slider to show what's there: `text` given how far
 * along, 0 to 1, while the pointer is over it, and `--point` as a percentage from its start edge.
 * Spread the handlers and style on the element.
 */
function usePointedAt(text: ((fraction: number) => string) | undefined) {
  const [at, setAt] = useState<number>();
  const style: CSSProperties & Record<"--point", string> = { "--point": `${(at ?? 0) * 100}%` };
  return {
    text: at === undefined ? undefined : text?.(at),
    style,
    onPointerMove: ({ currentTarget, clientX }: PointerEvent<HTMLElement>) => {
      setAt(fractionAt(currentTarget, clientX));
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
// `pointText` it lights the track up to the pointer and says what's there.
function Slider({
  className,
  label,
  valueText,
  buffered,
  pointText,
  ...props
}: SliderPrimitive.Root.Props<number> & {
  /** Names the slider for assistive technology. */
  label: string;
  /** The value in words, such as "4:05 of 1:42:10". */
  valueText?: string;
  /** How far along the track is loaded, in the slider's units, drawn under the fill. */
  buffered?: number;
  /** What's at the pointer, given how far along the track it is, 0 to 1, such as "21:05". */
  pointText?: (fraction: number) => string;
}) {
  const { text, ...pointer } = usePointedAt(pointText);
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
        <SliderPrimitive.Track
          data-slot="slider-track"
          className="relative h-1 grow overflow-hidden rounded-full bg-foreground/25 transition-all duration-200 ease-out select-none group-hover/slider:h-1.5 motion-reduce:transition-none"
        >
          {buffered !== undefined && (
            <progress
              aria-hidden
              value={buffered}
              max={props.max}
              className="absolute inset-0 size-full appearance-none bg-transparent [&::-moz-progress-bar]:bg-foreground/25 [&::-webkit-progress-bar]:bg-transparent [&::-webkit-progress-value]:bg-foreground/25"
            />
          )}
          {text !== undefined && (
            <div
              aria-hidden
              className="absolute inset-y-0 inset-s-0 w-(--point) bg-foreground/25 group-data-disabled/slider:hidden"
            />
          )}
          <SliderPrimitive.Indicator data-slot="slider-range" className="h-full bg-primary" />
        </SliderPrimitive.Track>
        {/* Focus lands on the thumb's hidden range input, so the ring follows `has-focus-visible`. */}
        <SliderPrimitive.Thumb
          data-slot="slider-thumb"
          aria-label={label}
          aria-valuetext={valueText}
          className="relative block size-3.5 shrink-0 scale-75 rounded-full bg-primary ring-ring/50 transition duration-150 ease-out-expo select-none group-hover/slider:scale-100 after:absolute after:-inset-2 hover:ring-3 has-focus-visible:scale-100 has-focus-visible:ring-3 has-focus-visible:ring-ring data-dragging:scale-100 motion-reduce:transition-none data-disabled:pointer-events-none data-disabled:bg-muted-foreground"
        />
        {/* Pointer-only: the thumb's `aria-valuetext` already says where it is. */}
        {text !== undefined && (
          <span
            aria-hidden
            className="pointer-events-none absolute inset-s-(--point) bottom-full mb-1.5 -translate-x-1/2 rounded-md bg-popover px-1.5 py-0.5 text-xs font-medium text-popover-foreground tabular-nums shadow-md group-data-disabled/slider:hidden rtl:translate-x-1/2"
          >
            {text}
          </span>
        )}
      </SliderPrimitive.Control>
    </SliderPrimitive.Root>
  );
}

export { Slider };
