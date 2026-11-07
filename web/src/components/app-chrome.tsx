import { useMutation } from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";
import { Link, type LinkProps, useNavigate } from "@tanstack/react-router";
import type { ReactNode } from "react";

import { Button, buttonVariants } from "@/components/ui/button";
import { SignInService } from "@/gen/nama/v1/sign_in_pb";

/** The bar above every signed-in page: where to go, and signing out. Admin is for Admins only. */
export function AppChrome({ isAdmin }: { isAdmin: boolean }) {
  return (
    // Lined up with the page's column: each link's own padding makes up the rest of the inset.
    <header className="mx-auto flex w-full max-w-5xl flex-wrap items-center px-1.5 py-3 sm:gap-1 sm:px-2">
      <nav aria-label="Main" className="flex items-center sm:gap-1">
        <NavLink to="/" activeOptions={{ exact: true }}>
          Libraries
        </NavLink>
        <NavLink to="/requests">Requests</NavLink>
        <NavLink to="/settings">Settings</NavLink>
        {isAdmin && <NavLink to="/admin">Admin</NavLink>}
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
      className={buttonVariants({
        variant: "ghost",
        size: "bar",
        className: "aria-[current=page]:text-foreground",
      })}
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
      size="bar"
      disabled={signOut.isPending}
      onClick={() => {
        signOut.mutate({});
      }}
    >
      Sign out
    </Button>
  );
}
