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
 * label reads over it.
 */
function ProgressPill({
  label,
  children,
  ...props
}: ProgressPrimitive.Root.Props & { label: string }) {
  return (
    <ProgressPrimitive.Root
      data-slot="progress-pill"
      className="relative isolate inline-flex shrink-0 items-center gap-1.5 overflow-hidden rounded-full border px-2.5 py-0.5 text-sm"
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

export { Progress, ProgressPill };
