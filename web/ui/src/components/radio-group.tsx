import { Radio as RadioPrimitive } from "@base-ui/react/radio";
import { RadioGroup as RadioGroupPrimitive } from "@base-ui/react/radio-group";
import { Check } from "lucide-react";

// The player's lists of choices, as its artboards draw them: 40px rows with 18px corners, the
// chosen one lit (Accent, Near-white ink and a Near-white rim at 60%) with a check at its end. One
// that can't be chosen is dimmed to muted ink.

/** Holds the rows of which one is chosen at a time; the arrow keys move between them. */
function RadioGroup(props: RadioGroupPrimitive.Props) {
  return (
    <RadioGroupPrimitive data-slot="radio-group" className="flex flex-col gap-0.5" {...props} />
  );
}

/** A row of a `RadioGroup`. */
function RadioRow({ children, ...props }: RadioPrimitive.Root.Props) {
  return (
    <RadioPrimitive.Root
      data-slot="radio-row"
      className="flex min-h-10 w-full shrink-0 cursor-pointer items-center gap-2.5 rounded-md px-2.5 text-start transition-colors outline-none select-none hover:bg-muted focus-visible:ring-3 focus-visible:ring-ring/50 data-checked:bg-accent data-checked:font-medium data-checked:text-primary data-checked:inset-ring data-checked:inset-ring-primary/60 data-disabled:cursor-default data-disabled:text-muted-foreground data-disabled:hover:bg-transparent"
      {...props}
    >
      <span className="flex-1 py-2">{children}</span>
      <RadioPrimitive.Indicator>
        <Check aria-hidden className="size-4" />
      </RadioPrimitive.Indicator>
    </RadioPrimitive.Root>
  );
}

/**
 * Choices in one pill, of which one is chosen, the shape `TabsList` draws: for a choice that
 * isn't a view, such as a filter's. Its segments share its width.
 */
function RadioSegments(props: RadioGroupPrimitive.Props) {
  return (
    <RadioGroupPrimitive
      data-slot="radio-segments"
      className="flex h-9 w-full items-center gap-0.5 rounded-lg bg-muted p-0.75"
      {...props}
    />
  );
}

/** A segment of `RadioSegments`, raised on the Rim once chosen. */
function RadioSegment(props: RadioPrimitive.Root.Props) {
  return (
    <RadioPrimitive.Root
      data-slot="radio-segment"
      className="inline-flex h-full min-w-0 flex-1 cursor-pointer items-center justify-center rounded-md border border-transparent px-3 text-sm font-medium whitespace-nowrap transition-colors outline-none select-none focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/50 data-checked:border-input data-checked:bg-input/30 data-checked:shadow-xs"
      {...props}
    />
  );
}

export { RadioGroup, RadioRow, RadioSegment, RadioSegments };
