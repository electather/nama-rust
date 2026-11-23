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

/**
 * A state pill filled to its `value` behind its `label` and its icon, the `children`, such as a
 * Request downloading. The fill is Ink at 25%, a quieter step than a bar's Near-white, so the
 * label reads over it. `large` beside a title's Play, as tall as it.
 */
function ProgressPill({
  label,
  large = false,
  children,
  ...props
}: ProgressPrimitive.Root.Props & { label: string; large?: boolean }) {
  return (
    <ProgressPrimitive.Root
      data-slot="progress-pill"
      className={cn(
        "relative isolate inline-flex shrink-0 items-center overflow-hidden rounded-full",
        // Beside a title's Play, as tall as it, and filled, as a hairline would be lost over art.
        large
          ? "h-11 gap-2 bg-secondary ps-4.5 pe-5.5 text-base font-medium"
          : "gap-1.5 border px-2.5 py-0.5 text-sm",
      )}
      {...props}
    >
      {/* Sized, as Base UI's indicator inherits its height. */}
      <ProgressPrimitive.Track className="absolute inset-0 -z-10 h-full">
        <ProgressPrimitive.Indicator className="h-full bg-foreground/25 transition-all duration-250 ease-linear motion-reduce:transition-none" />
      </ProgressPrimitive.Track>
      {children}
      <ProgressPrimitive.Label>{label}</ProgressPrimitive.Label>
    </ProgressPrimitive.Root>
  );
}

/**
 * A ring filled clockwise to its `value`, as small as an icon, for where a bar would add a line,
 * such as in a Request's row. Near-white on a Muted track, as a bar is.
 */
function ProgressRing({ value, ...props }: ProgressPrimitive.Root.Props) {
  return (
    <ProgressPrimitive.Root
      data-slot="progress-ring"
      value={value}
      className="grid size-3.5 shrink-0"
      {...props}
    >
      <svg aria-hidden viewBox="0 0 16 16" className="-rotate-90">
        <circle cx="8" cy="8" r="6.5" fill="none" strokeWidth="2" className="stroke-muted" />
        {/* `pathLength` makes the circle 100 long, so the dash's offset is what's left to go. */}
        <circle
          cx="8"
          cy="8"
          r="6.5"
          fill="none"
          strokeWidth="2"
          pathLength={100}
          strokeDasharray={100}
          strokeDashoffset={100 - (value ?? 0)}
          className="stroke-primary transition-all duration-250 ease-linear motion-reduce:transition-none"
        />
      </svg>
    </ProgressPrimitive.Root>
  );
}

export { Progress, ProgressPill, ProgressRing };
