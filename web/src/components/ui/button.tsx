import { Button as ButtonPrimitive } from "@base-ui/react/button";
import { cva, type VariantProps } from "class-variance-authority";
import { cn } from "cn";

// DESIGN.md, "Buttons": 10px radius, 14px medium text. Disabled is Muted with Quiet ink, never faded.
const buttonVariants = cva(
  "inline-flex h-9 shrink-0 items-center justify-center gap-1.5 rounded-lg px-4 text-sm font-medium whitespace-nowrap transition-colors outline-none select-none focus-visible:ring-3 focus-visible:ring-ring/50 disabled:pointer-events-none disabled:bg-muted disabled:text-muted-foreground",
  {
    variants: {
      variant: {
        // The one primary action on a screen.
        default: "bg-primary font-semibold text-primary-foreground hover:bg-primary/90",
        ghost: "text-muted-foreground hover:bg-muted hover:text-foreground",
      },
    },
    defaultVariants: { variant: "default" },
  },
);

function Button({
  className,
  variant = "default",
  ...props
}: ButtonPrimitive.Props & VariantProps<typeof buttonVariants>) {
  return (
    <ButtonPrimitive
      data-slot="button"
      className={cn(buttonVariants({ variant, className }))}
      {...props}
    />
  );
}

export { Button, buttonVariants };
