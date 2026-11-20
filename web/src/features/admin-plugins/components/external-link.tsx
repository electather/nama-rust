import type { ReactNode } from "react";

/**
 * A link out of the app to `href`, an http or https URL (`webUrl`), in a new tab that can’t reach
 * this one (`src/features/admin-plugins/AGENTS.md`).
 */
export function ExternalLink({ href, children }: { href: string; children: ReactNode }) {
  return (
    <a
      href={href}
      target="_blank"
      rel="noopener noreferrer"
      className="rounded-sm underline decoration-muted-foreground underline-offset-4 outline-none hover:decoration-foreground focus-visible:ring-3 focus-visible:ring-ring/50"
    >
      {children}
    </a>
  );
}
