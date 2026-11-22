import { Link, type LinkOptions } from "@tanstack/react-router";
import type { VariantProps } from "class-variance-authority";
import { cn } from "cn";
import { buttonVariants } from "nama-ui/components/button";
import type { ReactNode } from "react";

/** A link back to the title page. */
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
  return (
    <Link {...back} aria-label={label} className={cn(buttonVariants(variantProps))}>
      {children}
    </Link>
  );
}
