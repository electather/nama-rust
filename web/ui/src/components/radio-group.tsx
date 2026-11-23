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

export { RadioGroup, RadioRow };
