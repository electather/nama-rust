import { Slider as SliderPrimitive } from "@base-ui/react/slider";
import { cn } from "cn";

// DESIGN.md, "Sliders": a rimmed `well`, filled with `primary-tint` up to a `primary` block handle
// that stays inside it. Restyled from shadcn's to one value, so its thumb can carry the label.
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
      className={cn(
        "h-8 w-full rounded-md border border-input bg-well p-0.5 outline-offset-2 outline-ring has-focus-visible:outline-2 data-disabled:opacity-50",
        className,
      )}
      data-slot="slider"
      thumbAlignment="edge"
      {...props}
    >
      <SliderPrimitive.Control className="relative flex size-full touch-none items-center select-none">
        <SliderPrimitive.Track data-slot="slider-track" className="h-full grow select-none">
          <SliderPrimitive.Indicator
            data-slot="slider-range"
            className="rounded-sm bg-primary-tint"
          />
        </SliderPrimitive.Track>
        {/* Focus lands on the thumb's hidden range input, so the ring follows `has-focus-visible`. */}
        <SliderPrimitive.Thumb
          data-slot="slider-thumb"
          aria-label={label}
          aria-valuetext={valueText}
          className="block h-full w-2 shrink-0 rounded-sm bg-primary select-none after:absolute after:-inset-2 data-disabled:pointer-events-none"
        />
      </SliderPrimitive.Control>
    </SliderPrimitive.Root>
  );
}

export { Slider };
