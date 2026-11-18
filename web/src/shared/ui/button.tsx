import { Button as ButtonPrimitive } from "@base-ui/react/button";
import { cva, type VariantProps } from "class-variance-authority";
import { cn } from "cn";

// DESIGN.md, "Buttons": fully rounded, 32px, 14px medium text; disabled fades to half strength.
const buttonVariants = cva(
  "inline-flex shrink-0 items-center justify-center gap-1.5 rounded-full border border-transparent bg-clip-padding text-sm font-medium whitespace-nowrap transition-colors ease-out-expo outline-none select-none focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/50 active:translate-y-px disabled:pointer-events-none disabled:opacity-50 aria-disabled:pointer-events-none aria-disabled:opacity-50 motion-reduce:transition-none [&_svg]:shrink-0 [&_svg:not([class*='size-'])]:size-4",
  {
    variants: {
      variant: {
        // The one primary action on a screen.
        default: "bg-primary text-primary-foreground hover:bg-primary/80",
        // Cancel in a dialog, or a second way to do the same thing.
        outline: "border-input bg-input/30 text-foreground hover:bg-input/50",
        // Confirms something that can't be undone, such as deleting a User.
        destructive:
          "bg-destructive/10 text-destructive hover:bg-destructive/20 focus-visible:border-destructive/40 focus-visible:ring-destructive/40",
        ghost: "text-foreground-soft hover:bg-muted/50 hover:text-foreground",
        // Over video, where soft ink would sink into the picture and a faint ring could vanish.
        overlay: "text-foreground hover:bg-secondary focus-visible:ring-ring",
      },
      size: {
        default: "h-8 px-2.5",
        // The app bar's links: tighter on phones, so five fit across 390px; narrower, the bar wraps.
        bar: "h-9 px-2.5 sm:px-3.5",
        // A round button holding only an icon, named by its `aria-label`.
        icon: "size-8",
        // Play or Resume at the top of a title's page.
        pill: "h-11 px-5 text-base [&_svg]:fill-current",
      },
    },
    defaultVariants: { variant: "default", size: "default" },
  },
);

function Button({
  className,
  variant = "default",
  size = "default",
  ...props
}: ButtonPrimitive.Props & VariantProps<typeof buttonVariants>) {
  return (
    <ButtonPrimitive
      data-slot="button"
      className={cn(buttonVariants({ variant, size, className }))}
      {...props}
    />
  );
}

export { Button, buttonVariants };
