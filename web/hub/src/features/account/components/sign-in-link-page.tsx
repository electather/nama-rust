import { ExpiredSignInLink } from "@hub/features/account/components/expired-sign-in-link";
import { OpeningSignInLink } from "@hub/features/account/components/opening-sign-in-link";
import { SignInWithLink } from "@hub/features/account/components/sign-in-with-link";
import { useSignInLink, useSignInWithLink } from "@hub/features/account/data/sign-in-links";
import { refusalReason } from "@hub/features/account/lib/refusals";
import { RefusalReason } from "@hub/gen/nama/hub/v1/refusal_pb";

/**
 * The page an emailed Sign-in Link opens: which Account it signs in, and a button that does.
 * Opening it uses nothing up, since mail scanners open every link (ADR 0100).
 */
export function SignInLinkPage({ token }: { token: string }) {
  const link = useSignInLink(token);
  const use = useSignInWithLink();
  const expired = [link.error, use.error].some(
    (error) => refusalReason(error) === RefusalReason.SIGN_IN_LINK_EXPIRED,
  );
  if (expired) {
    return <ExpiredSignInLink />;
  }
  if (!link.data?.account) {
    return <OpeningSignInLink failed={link.error !== null} />;
  }
  return (
    <SignInWithLink
      account={link.data.account}
      emailProven={link.data.emailProven}
      pending={use.isPending}
      failed={use.error !== null}
      onSignIn={() => {
        use.mutate({ token });
      }}
    />
  );
}
