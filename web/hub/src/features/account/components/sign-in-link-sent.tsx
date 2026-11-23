import { PageHeading } from "@hub/app/page-heading";

/** Where to look for the link: the same words whether or not an Account has `email`. */
export function SignInLinkSent({ email }: { email: string | undefined }) {
  return (
    <>
      <PageHeading>Check your email</PageHeading>
      <output className="text-center text-base text-pretty">
        If an Account has <span className="break-all">{email}</span>, a Sign-in Link is on its way.
        It works once, within 15 minutes.
      </output>
    </>
  );
}
