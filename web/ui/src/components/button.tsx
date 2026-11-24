import { Button as ButtonPrimitive } from "@base-ui/react/button";
import { cva, type VariantProps } from "class-variance-authority";
import { cn } from "cn";

// DESIGN.md, "Buttons": a pill, 14px medium text, 32px tall. Disabled is Muted with Quiet ink, never faded.
const buttonVariants = cva(
  "inline-flex shrink-0 cursor-pointer items-center justify-center gap-1.5 rounded-full border border-transparent bg-clip-padding text-sm font-medium whitespace-nowrap transition-colors outline-none select-none focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/50 disabled:pointer-events-none disabled:bg-muted disabled:text-muted-foreground",
  {
    variants: {
      variant: {
        // The one primary action on a screen.
        default: "bg-primary font-semibold text-primary-foreground hover:bg-primary/80",
        // Confirms something that can't be undone, such as deleting a User. Red ink on its own tint:
        // `/10`, not shadcn's dark `/20`, which drops the ink below 4.5:1 on a popover.
        destructive:
          "bg-destructive/10 text-destructive hover:bg-destructive/20 focus-visible:border-destructive/40 focus-visible:ring-destructive/40",
        // A second action beside the primary one, such as "Sign in with a code": the Rim and its 30% fill.
        outline: "border-input bg-input/30 text-foreground hover:bg-input/50",
        // A second action beside the primary one over artwork, such as the player's Back and Episodes.
        // Lit while its panel is open or it's the choice made, such as the speed preset playing.
        secondary:
          "bg-secondary text-secondary-foreground hover:bg-muted aria-expanded:bg-accent aria-expanded:text-primary aria-expanded:inset-ring aria-expanded:inset-ring-primary/60 aria-pressed:bg-accent aria-pressed:text-primary aria-pressed:inset-ring aria-pressed:inset-ring-primary/60",
        ghost: "text-foreground hover:bg-muted/50",
        // A quiet text button beside a field's label, such as "Forgot your password?".
        link: "font-normal text-foreground underline-offset-3 hover:underline focus-visible:underline",
        // Over video, where Quiet ink would sink into the picture and a faint ring could vanish.
        // Lit while its panel is open or its mode is on, as the player's artboards draw it.
        overlay:
          "text-foreground hover:bg-foreground/15 focus-visible:ring-ring aria-expanded:bg-accent aria-expanded:text-primary aria-expanded:inset-ring aria-expanded:inset-ring-primary/60 aria-pressed:bg-accent aria-pressed:text-primary aria-pressed:inset-ring aria-pressed:inset-ring-primary/60",
      },
      size: {
        // An icon beside the words is 16px, as DESIGN.md's buttons have it, unless it sets its own size.
        // `ghostHang` pulls back its `px-2.5`, and `ghostInset` indents by it.
        default: "h-8 px-2.5 [&_svg:not([class*='size-'])]:size-4",
        // The full-width actions of a sign-in card.
        lg: "h-9 px-2.5 [&_svg]:size-4",
        // Text only, with a 44px tall hit area that its negative margins keep out of the layout.
        link: "-my-3 h-auto rounded-sm px-0 py-3",
        // The app bar's links: tighter on phones, so five fit across 390px; narrower, the bar wraps.
        bar: "h-9 px-2.5 sm:px-4",
        // A dialog's ✕ in its corner, quiet until hovered or focused.
        close:
          "size-7 rounded-sm opacity-70 hover:opacity-100 focus-visible:opacity-100 [&_svg]:size-4",
        // A square button holding only an icon, named by its `aria-label`: the player's controls.
        icon: "size-10 rounded-lg [&_svg]:size-5",
        // The same, everywhere but over video: a dialog's ✕, a row's "…", a list's move buttons.
        "icon-sm": "size-8 rounded-lg [&_svg]:size-4",
        // The player's large controls, 44px tall: `round` holds only an icon.
        round: "size-11 [&_svg]:size-5",
        tall: "h-11 px-5 [&_svg]:size-5",
        // `tall` whose words its caller hides on a phone, so it's `round` there: the player's Episodes.
        "round-on-phone": "size-11 sm:w-auto sm:px-5 [&_svg]:size-5",
        // Play or Resume at the top of a title's page, as a pill.
        pill: "h-11 px-6 text-base [&_svg]:size-4 [&_svg]:fill-current",
        // A Season's name over its Episodes, which opens the menu that picks another Season.
        heading: "h-11 ps-5 pe-4 text-lg font-semibold [&_svg]:size-5",
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

/**
 * Pulls a default-size ghost button back by its own padding, so its words line up with the text
 * above. Must equal the `default` size's `px-2.5`.
 */
const ghostHang = "-ms-2.5";

/**
 * Indents text under a default-size ghost button by its padding, so the text lines up with
 * the button's words. Must equal the `default` size's `px-2.5`.
 */
const ghostInset = "ms-2.5";

export { Button, buttonVariants, ghostHang, ghostInset };
