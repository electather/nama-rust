import { LinkCodeForm } from "@/features/auth/components/link-code-form";
import { useApproveUserLink } from "@/features/auth/data/use-approve-user-link";

/** Entering the code a User's Server shows them, which Links that User to the signed-in Account. */
export function LinkUserForm() {
  const approve = useApproveUserLink();
  const user = approve.data?.user;
  return (
    <LinkCodeForm
      // A User's Settings tells them to enter the code under this heading.
      heading="Link a User"
      hint="Your Server shows it in Settings, under Sign-in."
      action="Link User"
      linked={
        user === undefined
          ? undefined
          : `${user.name} on ${user.serverName} is Linked to your Account.`
      }
      pending={approve.isPending}
      error={approve.error}
      onCode={(code, form) => {
        approve.mutate(
          { code },
          {
            onSuccess: () => {
              form.reset();
            },
          },
        );
      }}
    />
  );
}
