import { PageHeading } from "@hub/app/page-heading";
import { useApproveServerSignIn } from "@hub/features/links/data/server-sign-ins";
import { serverSignInError } from "@hub/features/links/lib/server-sign-in-refusal";
import { Button } from "nama-ui/components/button";

/** Approving the sign-in `id` to `serverName` as the Account's User there, `userName`. */
export function ApproveServerSignIn({
  id,
  serverName,
  userName,
}: {
  id: string;
  serverName: string;
  userName: string;
}) {
  const approve = useApproveServerSignIn();
  // Once approved, the browser is on its way back to the Server.
  const going = approve.isPending || approve.isSuccess;
  return (
    <>
      <PageHeading>{`Sign in to ${serverName}`}</PageHeading>
      <p className="text-center text-sm text-pretty text-muted-foreground">
        You’ll sign in as {userName}. Only continue if you just chose to sign in on {serverName}.
      </p>
      <p role="alert" className="text-sm text-destructive empty:hidden">
        {approve.error ? serverSignInError(approve.error) : undefined}
      </p>
      <Button
        size="lg"
        disabled={going}
        onClick={() => {
          approve.mutate({ id });
        }}
      >
        {going ? `Going back to ${serverName}…` : `Sign in as ${userName}`}
      </Button>
    </>
  );
}
