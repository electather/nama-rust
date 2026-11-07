import { Code } from "@connectrpc/connect";
import { Link, type ErrorComponentProps, useRouter } from "@tanstack/react-router";

import { Page } from "@/components/page";
import { Button, buttonVariants } from "@/components/ui/button";
import { isCode } from "@/lib/connect-errors";

/** What a page shows instead when it fails, in plain words and never an error code. */
export function RouteError({ error }: ErrorComponentProps) {
  const router = useRouter();
  if (isCode(error, Code.PermissionDenied)) return <AdminsOnly />;
  // The router is already rechecking the session, and sends a signed-out visitor to /sign-in.
  if (isCode(error, Code.Unauthenticated)) return null;
  return (
    <Page heading="Something went wrong">
      <p className="mt-2 text-muted-foreground">
        This page couldn’t load. Check that the Server is running, then try again.
      </p>
      <div className="mt-6">
        <Button onClick={() => void router.invalidate()}>Try again</Button>
      </div>
    </Page>
  );
}

function AdminsOnly() {
  return (
    <Page heading="Admins only">
      <p className="mt-2 max-w-prose text-muted-foreground">
        Only an Admin of this Server can open this page. If you need something changed here, ask
        whoever runs the Server.
      </p>
      {/* Pulled back by the button's own padding, so its words line up with the text above. */}
      <div className="mt-6 -ms-4">
        <Link to="/" className={buttonVariants({ variant: "ghost" })}>
          Back to your Libraries
        </Link>
      </div>
    </Page>
  );
}

export function NotFound() {
  return (
    <Page heading="Page not found">
      <p className="mt-2 text-muted-foreground">There’s nothing at this address.</p>
    </Page>
  );
}
