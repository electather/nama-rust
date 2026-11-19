import { Progress as ProgressPrimitive } from "@base-ui/react/progress";
import { cn } from "cn";

// DESIGN.md, "Sliders and progress": a 2px bar, Near-white on a Muted track. Trimmed to the parts we use.
function Progress({ className, ...props }: ProgressPrimitive.Root.Props) {
  return (
    <ProgressPrimitive.Root data-slot="progress" className={cn("w-full", className)} {...props}>
      <ProgressPrimitive.Track
        data-slot="progress-track"
        className="relative h-0.5 w-full overflow-hidden rounded-full bg-muted"
      >
        {/* A video reports its time about every 250ms, so the fill glides over that gap instead of
            jumping, as in the Next Episode countdown. */}
        <ProgressPrimitive.Indicator
          data-slot="progress-indicator"
          className="h-full rounded-full bg-primary transition-all duration-250 ease-linear motion-reduce:transition-none"
        />
      </ProgressPrimitive.Track>
    </ProgressPrimitive.Root>
  );
}

export { Progress };
