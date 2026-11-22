import { Link, Outlet } from "@tanstack/react-router";
import { buttonVariants } from "nama-ui/components/button";

import type { adminPagesFor } from "@/shared/hooks/access";

/** Admin pages, under a nav of the ones the User may open. */
export function AdminLayout({
  allowedAdminPages,
}: {
  allowedAdminPages: ReturnType<typeof adminPagesFor>;
}) {
  return (
    <>
      {/* Lined up with the page's column: each link's own padding makes up the rest of the inset. */}
      <nav aria-label="Admin" className="mx-auto flex w-full max-w-5xl flex-wrap gap-1 sm:px-2">
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
