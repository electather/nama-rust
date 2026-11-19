import { Slider as SliderPrimitive } from "@base-ui/react/slider";
import { cn } from "cn";

// DESIGN.md, "Sliders and progress": a Muted track, a Near-white fill and thumb, and the focus ring
// at full strength, since the slider sits over video. A disabled thumb is Quiet ink, never faded,
// as DESIGN.md says of a disabled button. Restyled from shadcn's to one value, so its thumb can
// carry the label.
function Slider({
  className,
  label,
  valueText,
  ...props
}: SliderPrimitive.Root.Props<number> & {
  /** Names the slider for assistive technology. */
  label: string;
  /** The value in words, such as "4:05 of 1:42:10". */
  valueText?: string;
}) {
  return (
    <SliderPrimitive.Root
      className={cn("w-full", className)}
      data-slot="slider"
      thumbAlignment="edge"
      {...props}
    >
      <SliderPrimitive.Control className="relative flex h-6 w-full touch-none items-center select-none">
        <SliderPrimitive.Track
          data-slot="slider-track"
          className="relative h-1 grow overflow-hidden rounded-full bg-muted select-none"
        >
          <SliderPrimitive.Indicator data-slot="slider-range" className="h-full bg-primary" />
        </SliderPrimitive.Track>
        {/* Focus lands on the thumb's hidden range input, so the ring follows `has-focus-visible`. */}
        <SliderPrimitive.Thumb
          data-slot="slider-thumb"
          aria-label={label}
          aria-valuetext={valueText}
          className="relative block size-3.5 shrink-0 rounded-full bg-primary ring-ring/50 transition-shadow select-none after:absolute after:-inset-2 hover:ring-3 has-focus-visible:ring-3 has-focus-visible:ring-ring motion-reduce:transition-none data-disabled:pointer-events-none data-disabled:bg-muted-foreground"
        />
      </SliderPrimitive.Control>
    </SliderPrimitive.Root>
  );
}

export { Slider };
