import { useMutation } from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";
import { Link, type LinkProps, useNavigate } from "@tanstack/react-router";
import type { ReactNode } from "react";

import { Button } from "@/components/ui/button";
import { SignInService } from "@/gen/nama/v1/sign_in_pb";

/** The bar above every signed-in page: where to go, and signing out. Admin is for Admins only. */
export function AppChrome({ isAdmin }: { isAdmin: boolean }) {
  return (
    // Lined up with the page's column: each link's own padding makes up the rest of the inset.
    <header className="mx-auto flex w-full max-w-5xl items-center gap-1 px-1 py-3 sm:px-3">
      <nav aria-label="Main" className="flex items-center gap-1">
        <NavLink to="/" activeOptions={{ exact: true }}>
          Libraries
        </NavLink>
        <NavLink to="/settings">Settings</NavLink>
        {isAdmin && <NavLink to="/admin/users">Admin</NavLink>}
      </nav>
      <div className="ms-auto">
        <SignOutButton />
      </div>
    </header>
  );
}

function NavLink(props: LinkProps & { children: ReactNode }) {
  return (
    <Link
      className="rounded-lg px-3 py-2 text-sm font-medium text-muted-foreground transition-colors outline-none hover:bg-muted hover:text-foreground focus-visible:ring-3 focus-visible:ring-ring/50 aria-[current=page]:text-foreground"
      {...props}
    />
  );
}

function SignOutButton() {
  const queryClient = useQueryClient();
  const navigate = useNavigate();
  const signOut = useMutation(SignInService.method.signOut, {
    onSuccess: async () => {
      // Nothing cached for the User who just left may be shown to whoever signs in next.
      queryClient.clear();
      await navigate({ to: "/sign-in" });
    },
  });
  return (
    <Button
      variant="ghost"
      disabled={signOut.isPending}
      onClick={() => {
        signOut.mutate({});
      }}
    >
      Sign out
    </Button>
  );
}
