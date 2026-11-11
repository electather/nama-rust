import { useMutation, useQuery } from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";
import { Link, type LinkProps, useLocation, useNavigate, useSearch } from "@tanstack/react-router";
import type { ReactNode } from "react";

import { Button, buttonVariants } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { RequestService } from "@/gen/nama/v1/request_pb";
import { SignInService } from "@/gen/nama/v1/sign_in_pb";

/**
 * The bar above every signed-in page: where to go, searching your Libraries, and signing out.
 * Admin shows only when the User may open some Admin page, and the Pending count only to a Request
 * manager.
 */
export function AppChrome({
  hasAdminPages,
  managesRequests,
}: {
  hasAdminPages: boolean;
  managesRequests: boolean;
}) {
  return (
    // Lined up with the page's column: each link's own padding makes up the rest of the inset.
    <header className="mx-auto flex w-full max-w-5xl flex-wrap items-center px-1.5 py-3 sm:gap-1 sm:px-2">
      <nav aria-label="Main" className="flex items-center sm:gap-1">
        <NavLink to="/" activeOptions={{ exact: true }}>
          Libraries
        </NavLink>
        <RequestsLink managesRequests={managesRequests} />
        <NavLink to="/settings">Settings</NavLink>
        {hasAdminPages && <NavLink to="/admin">Admin</NavLink>}
      </nav>
      <SearchBox />
      <div className="ms-auto sm:ms-0">
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

/**
 * The link to Requests, with how many are Pending for a Request manager. Request Events refetch the
 * count along with every other Request query.
 */
function RequestsLink({ managesRequests }: { managesRequests: boolean }) {
  const { data } = useQuery(
    RequestService.method.countPendingRequests,
    {},
    { enabled: managesRequests },
  );
  const pending = data?.count ?? 0;
  if (pending === 0) return <NavLink to="/requests">Requests</NavLink>;
  return (
    <NavLink to="/requests" aria-label={`Requests, ${pending} Pending`}>
      Requests
      <PendingBadge pending={pending} />
    </NavLink>
  );
}

// DESIGN.md "State labels": a hairline pill, so its edge stays when the ghost link fills on hover.
function PendingBadge({ pending }: { pending: number }) {
  return (
    <span className="ms-1.5 rounded-full border px-1.5 text-sm tabular-nums text-foreground">
      {pending > 99 ? "99+" : pending}
    </span>
  );
}

/**
 * Searches the signed-in User's Libraries as they type, on the search page, which keeps the query
 * in its URL. Typing there replaces its history entry, so Back leaves the search.
 */
function SearchBox() {
  const navigate = useNavigate();
  const onSearchPage = useLocation({ select: ({ pathname }) => pathname === "/search" });
  // Only the search page has a `query`. Read once: after that, only the box changes it.
  const { query } = useSearch({ strict: false });
  return (
    // On phones it takes a row of its own under the links, its edge lined up with theirs.
    <search className="order-last mt-2 w-full px-2.5 sm:order-none sm:ms-auto sm:mt-0 sm:w-56 sm:px-0">
      <Input
        type="search"
        aria-label="Search your Libraries"
        placeholder="Search"
        autoComplete="off"
        defaultValue={query}
        onChange={(event) => {
          const typed = event.target.value.trim();
          void navigate({
            to: "/search",
            search: typed === "" ? {} : { query: typed },
            replace: onSearchPage,
          });
        }}
      />
    </search>
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
