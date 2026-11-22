import { Code } from "@connectrpc/connect";
import { type ErrorComponentProps, useRouter } from "@tanstack/react-router";
import { Button } from "nama-ui/components/button";

import { NotAllowed } from "@/app/route-error/not-allowed";
import { Page } from "@/shared/components/page";
import { isCode } from "@/shared/lib/connect-errors";

/** What a page shows instead when it fails, in plain words and never an error code. */
export function RouteError({ error }: ErrorComponentProps) {
  const router = useRouter();
  if (isCode(error, Code.PermissionDenied)) {
    return <NotAllowed />;
  }
  // The router is already rechecking the session, and sends a signed-out visitor to /sign-in.
  if (isCode(error, Code.Unauthenticated)) {
    return null;
  }
  return (
    <Page heading="Something went wrong">
      <p className="mt-2 text-muted-foreground">
        This page couldn’t load. Check that the Server is running, then try again.
      </p>
      <div className="mt-6">
        <Button
          onClick={() => {
            void router.invalidate();
          }}
        >
          Try again
        </Button>
      </div>
    </Page>
  );
}
