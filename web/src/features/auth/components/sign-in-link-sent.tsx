import { SignInCard } from "@/shared/components/sign-in-layout/sign-in-card";

/** Where to look for the link: the same words whether or not an Account has `email`. */
export function SignInLinkSent({ email }: { email: string }) {
  return (
    <SignInCard heading="Check your email">
      <p className="text-center text-base text-pretty">
        If an Account has <span className="break-all">{email}</span>, a Sign-in Link is on its way.
        It works once, within 15 minutes.
      </p>
    </SignInCard>
  );
}
