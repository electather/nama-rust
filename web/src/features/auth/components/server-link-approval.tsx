import { useLocation } from "@tanstack/react-router";

import { LinkRemedy } from "@/features/auth/components/link-remedy";
import { useApproveServerLink } from "@/features/auth/data/use-approve-server-link";
import { linkApprovalErrors } from "@/features/auth/lib/link-errors";
import { SignInLayout } from "@/shared/components/sign-in-layout/sign-in-layout";

/**
 * The Hub's page a Server sends its Admin to, to Link it to their Account by redirect, without
 * typing the `code` it shows (ADR 0105). Approving sends the browser back to the Server.
 */
export function ServerLinkApproval({ code }: { code: string }) {
  const approve = useApproveServerLink();
  const here = useLocation({ select: (location) => location.href });
  // Once approved, the browser is on its way back to the Server.
  const going = approve.isPending || approve.isSuccess;
  return (
    <SignInLayout
      heading="Link a Server"
      action={approvalAction(going, approve.data?.server?.name)}
      pending={going}
      errors={linkApprovalErrors(approve.error)}
      footer={<LinkRemedy error={approve.error} redirect={here} />}
      onSubmit={() => {
        approve.mutate({ code }, { onSuccess: goBack });
      }}
    >
      <p className="text-center text-sm text-pretty text-muted-foreground">
        It becomes a Linked Server of your Account. Only continue if you just chose to Link it in
        your Server’s Settings.
      </p>
    </SignInLayout>
  );
}

/** What the button says: once `linked` names the Server, that the browser goes back to it. */
function approvalAction(pending: boolean, linked: string | undefined) {
  if (linked !== undefined) {
    return `Going back to ${linked}…`;
  }
  return pending ? "Linking…" : "Link Server";
}

/** Sends the browser to where the Server started Linking, or the Account's page without one. */
function goBack({ returnUrl }: { returnUrl: string }) {
  globalThis.location.assign(returnUrl === "" ? "/" : returnUrl);
}
