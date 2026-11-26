import { Link, type LinkOptions } from "@tanstack/react-router";
import type { VariantProps } from "class-variance-authority";
import { cn } from "cn";
import { buttonVariants } from "nama-ui/components/button";
import type { ReactNode } from "react";

import { useGoBack } from "@/features/player/hooks/use-go-back";

/** The player's link back, which leaves it as `useGoBack` does, to the page that opened it or the title page. */
export function BackLink({
  back,
  label,
  children,
  ...variantProps
}: VariantProps<typeof buttonVariants> & {
  back: LinkOptions;
  label?: string;
  children: ReactNode;
}) {
  const goBack = useGoBack(back);
  return (
    <Link
      {...back}
      aria-label={label}
      className={cn(buttonVariants(variantProps))}
      onClick={(event) => {
        // A modified click opens the title page in a new tab or window, as any link's does.
        const { button, metaKey, ctrlKey, shiftKey, altKey } = event;
        if (button === 0 && ![metaKey, ctrlKey, shiftKey, altKey].includes(true)) {
          event.preventDefault();
          goBack();
        }
      }}
    >
      {children}
    </Link>
  );
}
