import { Button as ButtonPrimitive } from "@base-ui/react/button";
import { cva, type VariantProps } from "class-variance-authority";
import { cn } from "cn";

// DESIGN.md, "Buttons": 10px radius, 14px medium text. Disabled is Muted with Quiet ink, never faded.
const buttonVariants = cva(
  "inline-flex shrink-0 items-center justify-center gap-1.5 rounded-lg text-sm font-medium whitespace-nowrap transition-colors outline-none select-none focus-visible:ring-3 focus-visible:ring-ring/50 disabled:pointer-events-none disabled:bg-muted disabled:text-muted-foreground",
  {
    variants: {
      variant: {
        // The one primary action on a screen.
        default: "bg-primary font-semibold text-primary-foreground hover:bg-primary/90",
        // Confirms something that can't be undone, such as deleting a User.
        destructive: "bg-destructive font-semibold text-background hover:bg-destructive/90",
        ghost: "text-muted-foreground hover:bg-muted hover:text-foreground",
        // Over video, where Quiet ink would sink into the picture and a faint ring could vanish.
        overlay: "text-foreground hover:bg-foreground/15 focus-visible:ring-ring",
      },
      size: {
        default: "h-9 px-4",
        // The app bar's links: tighter on phones, so five fit across 390px; narrower, the bar wraps.
        bar: "h-9 px-2.5 sm:px-4",
        // A square button holding only an icon, named by its `aria-label`.
        icon: "size-10 [&_svg]:size-5",
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
