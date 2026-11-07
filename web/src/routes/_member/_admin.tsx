import { Code, ConnectError } from "@connectrpc/connect";
import { Link, Outlet, createFileRoute } from "@tanstack/react-router";

import { buttonVariants } from "@/components/ui/button";
import { isAdmin } from "@/lib/users";

/** Pages for Admins. Anyone else gets the same "Admins only" page the Server's own refusal shows. */
export const Route = createFileRoute("/_member/_admin")({
  beforeLoad: ({ context: { signedIn } }) => {
    if (!isAdmin(signedIn.user)) {
      throw new ConnectError("Admins only", Code.PermissionDenied);
    }
  },
  component: AdminLayout,
});

const pages = [
  { to: "/admin/libraries", label: "Libraries" },
  { to: "/admin/needs-a-match", label: "Needs a Match" },
  { to: "/admin/users", label: "Users" },
] as const;

function AdminLayout() {
  return (
    <>
      {/* Lined up with the page's column: each link's own padding makes up the rest of the inset. */}
      <nav aria-label="Admin" className="mx-auto flex w-full max-w-5xl flex-wrap gap-1 sm:px-2">
        {pages.map(({ to, label }) => (
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
