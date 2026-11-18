import { Code, ConnectError } from "@connectrpc/connect";
import { createFileRoute } from "@tanstack/react-router";

import { AdminLayout } from "@/app/admin-layout";
import { adminPages, adminPagesFor } from "@/shared/hooks/access";

// src/routes/_member/_admin/AGENTS.md, "Admin pages need a Permission".
/**
 * Admin pages, each for Users with the Permission `adminPages` gives it. Anyone else gets the same
 * page the Server's own refusal shows.
 */
export const Route = createFileRoute("/_member/_admin")({
  beforeLoad: ({ context: { signedIn }, location }) => {
    const allowedAdminPages = adminPagesFor(signedIn.user);
    // ponytail: matched by path prefix, since no page's path starts another's; give each route a
    // `staticData` Permission instead once one does.
    const page = adminPages.find(({ to }) => location.pathname.startsWith(to));
    // `/admin` itself needs some page. Any other page missing from `adminPages` opens for nobody.
    const mayOpen = page
      ? allowedAdminPages.includes(page)
      : /^\/admin\/?$/u.test(location.pathname) && allowedAdminPages.length > 0;
    if (!mayOpen) {
      throw new ConnectError("this page needs a Permission you don't have", Code.PermissionDenied);
    }
    return { allowedAdminPages };
  },
  component: Admin,
});

function Admin() {
  const { allowedAdminPages } = Route.useRouteContext();
  return <AdminLayout allowedAdminPages={allowedAdminPages} />;
}
