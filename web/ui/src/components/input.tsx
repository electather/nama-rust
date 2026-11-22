import { Input as InputPrimitive } from "@base-ui/react/input";
import { cn } from "cn";
import type { ComponentProps } from "react";

// DESIGN.md, "Inputs": the Rim at 30% as a fill, a full Rim border, 10px radius, Near-white on focus.
function Input({
  className,
  type,
  variant = "default",
  ...props
}: ComponentProps<"input"> & {
  /** `lg` for the fields of a sign-in card, as tall as its buttons, and `code` for a code typed there. */
  variant?: "default" | "lg" | "code";
}) {
  return (
    <InputPrimitive
      type={type}
      data-slot="input"
      className={cn(
        "w-full min-w-0 rounded-lg border border-input bg-input/30 px-2.5 text-base transition-colors outline-none placeholder:text-muted-foreground focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/50 disabled:cursor-not-allowed disabled:bg-muted aria-invalid:border-destructive/50 aria-invalid:ring-3 aria-invalid:ring-destructive/40",
        variant === "default" ? "h-8" : "h-9",
        variant === "code" && "text-center tracking-widest tabular-nums",
        className,
      )}
      {...props}
    />
  );
}

export { Input };
