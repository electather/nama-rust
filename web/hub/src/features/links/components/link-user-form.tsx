import { CodeForm } from "@hub/features/links/components/code-form";
import { useApproveUserLink } from "@hub/features/links/data/users";

/** Entering the code a User's Server shows them, which Links that User to the signed-in Account. */
export function LinkUserForm() {
  const approve = useApproveUserLink();
  const user = approve.data?.user;
  return (
    <CodeForm
      id="link-user"
      heading="Link a User"
      hint="Your Server shows it under Settings, Sign-in, once you choose Link to the Hub there."
      action="Link User"
      linked={
        user === undefined
          ? undefined
          : `${user.name} on ${user.serverName} is Linked to your Account.`
      }
      approve={{
        mutate: (code, done) => {
          approve.mutate({ code }, { onSuccess: done });
        },
        error: approve.error,
        isPending: approve.isPending,
      }}
    />
  );
}
