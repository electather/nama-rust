import { Input as InputPrimitive } from "@base-ui/react/input";
import { cn } from "cn";
import type { ComponentProps } from "react";

// DESIGN.md, "Inputs": an `input` rim over its 30% fill; focus turns the rim to `ring` with a 3px ring.
// 16px on phones, so iOS doesn't zoom into the field.
function Input({ className, type, ...props }: ComponentProps<"input">) {
  return (
    <InputPrimitive
      type={type}
      data-slot="input"
      className={cn(
        "h-8 w-full min-w-0 rounded-lg border border-input bg-input/30 px-2.5 text-base text-foreground transition ease-out-expo outline-none placeholder:text-muted-foreground focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/50 disabled:pointer-events-none disabled:opacity-50 aria-invalid:border-destructive/50 aria-invalid:ring-3 aria-invalid:ring-destructive/40 motion-reduce:transition-none md:text-sm",
        className,
      )}
      {...props}
    />
  );
}

export { Input };
