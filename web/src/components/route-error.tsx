import { Code } from "@connectrpc/connect";
import { Link, type ErrorComponentProps, useRouter } from "@tanstack/react-router";

import { Page } from "@/components/page";
import { Button, buttonVariants } from "@/components/ui/button";
import { RefusalReason } from "@/gen/nama/v1/refusal_pb";
import { isCode } from "@/lib/connect-errors";
import { refusalOf } from "@/lib/forms";

/** What a page shows instead when it fails, in plain words and never an error code. */
export function RouteError({ error }: ErrorComponentProps) {
  const router = useRouter();
  if (isCode(error, Code.PermissionDenied)) return <NotAllowed />;
  // The router is already rechecking the session, and sends a signed-out visitor to /sign-in.
  if (isCode(error, Code.Unauthenticated)) return null;
  // Only the player names a Version, so one that's gone was deleted while it played.
  if (refusalOf(error).reason === RefusalReason.VERSION_NOT_FOUND) return <DeletedWhilePlaying />;
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

function NotAllowed() {
  return (
    <Page heading="You can’t open this page">
      <p className="mt-2 max-w-prose text-muted-foreground">
        It needs a Permission you don’t have. If you need something changed here, ask whoever runs
        the Server.
      </p>
      <BackToLibraries />
    </Page>
  );
}

/** What the player shows once what it plays was deleted from the Server. */
export function DeletedWhilePlaying() {
  return (
    <Page heading="Deleted from the Server">
      <p className="mt-2 max-w-prose text-muted-foreground">
        Someone deleted what was playing, so it can’t play any more.
      </p>
      <BackToLibraries />
    </Page>
  );
}

function BackToLibraries() {
  return (
    // Pulled back by the button's own padding, so its words line up with the text above.
    <div className="mt-6 -ms-4">
      <Link to="/" className={buttonVariants({ variant: "ghost" })}>
        Back to your Libraries
      </Link>
    </div>
  );
}

export function NotFound() {
  return (
    <Page heading="Page not found">
      <p className="mt-2 text-muted-foreground">There’s nothing at this address.</p>
    </Page>
  );
}
