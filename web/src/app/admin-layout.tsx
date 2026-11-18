import { Link, Outlet } from "@tanstack/react-router";

import type { adminPagesFor } from "@/shared/hooks/access";
import { buttonVariants } from "@/shared/ui/button";

/** Admin pages, under a nav of the ones the User may open. */
export function AdminLayout({
  allowedAdminPages,
}: {
  allowedAdminPages: ReturnType<typeof adminPagesFor>;
}) {
  return (
    <>
      {/* Lined up with the page's column: each link's own padding makes up the rest of the inset. */}
      <nav
        aria-label="Admin"
        className="mx-auto flex w-full max-w-5xl flex-wrap gap-1 px-1.5 sm:px-3.5"
      >
        {allowedAdminPages.map(({ to, label }) => (
          <Link
            key={to}
            to={to}
            className={buttonVariants({
              variant: "ghost",
              className: "h-8 aria-[current=page]:bg-muted aria-[current=page]:text-foreground",
            })}
          >
            {label}
          </Link>
        ))}
      </nav>
      <Outlet />
    </>
  );
}
