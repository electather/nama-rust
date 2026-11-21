import { cn } from "cn";
import type { ReactNode } from "react";

import { textLink } from "@/shared/lib/text-link-classes";

/** An underlined link out of the app to `href`, in a new tab that can't reach this one. */
export function ExternalTextLink({
  href,
  className,
  children,
}: {
  href: string;
  className?: string;
  children: ReactNode;
}) {
  return (
    <a href={href} target="_blank" rel="noopener noreferrer" className={cn(textLink, className)}>
      {children}
    </a>
  );
}
