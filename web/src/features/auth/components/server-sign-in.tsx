import { useApproveServerSignIn } from "@/features/auth/data/use-approve-server-sign-in";
import { hubErrors } from "@/features/auth/lib/hub-errors";
import { SignInLayout } from "@/shared/components/sign-in-layout/sign-in-layout";

/**
 * The Hub's page a Server sends someone to, to sign in there with their Account as its User
 * `userName`. Approving sends the browser back to the Server.
 */
export function ServerSignIn({
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
    <SignInLayout
      heading={`Sign in to ${serverName}`}
      action={going ? `Going back to ${serverName}…` : `Sign in as ${userName}`}
      pending={going}
      errors={approve.error ? hubErrors(approve.error) : {}}
      onSubmit={() => {
        approve.mutate({ id });
      }}
    >
      <p className="text-center text-sm text-pretty text-muted-foreground">
        You’ll sign in as {userName}. Only continue if you just chose to sign in on {serverName}.
      </p>
    </SignInLayout>
  );
}
