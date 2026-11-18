import { InviteFields } from "@/features/invites/components/invite-fields";
import { useAcceptInvite } from "@/features/invites/data/use-accept-invite";
import { useServerName } from "@/features/invites/data/use-server-name";
import { acceptInviteErrors } from "@/features/invites/lib/invite-refusals";
import { SessionDelivery } from "@/gen/nama/v1/sign_in_pb";
import { SignInLayout } from "@/shared/components/sign-in-layout/sign-in-layout";
import { formText } from "@/shared/lib/forms";

/** The page an Invite's link opens, where someone picks the name and password of their new User. */
export function AcceptInvite({ token }: { token: string }) {
  const name = useServerName();
  const accept = useAcceptInvite();
  const errors = accept.error ? acceptInviteErrors(accept.error) : {};

  return (
    <SignInLayout
      heading={name === undefined ? "Join this Server" : `Join ${name}`}
      intro="You’ve been invited. Pick the name and password you’ll sign in with."
      action={accept.isPending ? "Joining…" : "Join"}
      pending={accept.isPending}
      error={errors.form}
      onSubmit={(form) => {
        accept.mutate({
          token,
          name: formText(form, "name"),
          password: formText(form, "password"),
          delivery: SessionDelivery.COOKIE,
        });
      }}
    >
      <InviteFields errors={errors} />
    </SignInLayout>
  );
}
