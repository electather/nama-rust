import { cn } from "cn";
import type * as React from "react";

// DESIGN.md, "Inputs", as for `Input`: transparent, 14% hairline, 10px radius, green on focus.
function Textarea({ className, ...props }: React.ComponentProps<"textarea">) {
  return (
    <textarea
      data-slot="textarea"
      className={cn(
        "min-h-24 w-full min-w-0 rounded-lg border border-input bg-transparent px-3 py-2 text-base transition-colors outline-none placeholder:text-muted-foreground focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/25 disabled:cursor-not-allowed disabled:bg-muted aria-invalid:border-destructive",
        className,
      )}
      {...props}
    />
  );
}

export { Textarea };
