import { ApproveServerSignIn } from "@hub/features/links/components/approve-server-sign-in";
import { NO_USER_ON_SERVER } from "@hub/features/links/lib/server-sign-in-refusal";

/** The sign-in `id` to `serverName`, as the Account's User there, `userName`, if it has one. */
export function ServerSignInAs({
  id,
  serverName,
  userName,
}: {
  id: string;
  serverName: string;
  /** Empty when the Account has no User there, which can't sign in to it. */
  userName: string;
}) {
  return userName === "" ? (
    <>
      <h1 className="text-center text-3xl font-medium tracking-tight text-balance">
        Sign in to {serverName}
      </h1>
      <p role="alert" className="text-center text-sm text-pretty text-destructive">
        {NO_USER_ON_SERVER}
      </p>
    </>
  ) : (
    <ApproveServerSignIn id={id} serverName={serverName} userName={userName} />
  );
}
