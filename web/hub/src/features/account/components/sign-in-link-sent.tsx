/** Where to look for the link: the same words whether or not an Account has `email`. */
export function SignInLinkSent({ email }: { email: string | undefined }) {
  return (
    <>
      <h1 className="text-center text-3xl font-medium tracking-tight text-balance">
        Check your email
      </h1>
      <output className="text-center text-base text-pretty">
        If an Account has <span className="break-all">{email}</span>, a Sign-in Link is on its way.
        It works once, within 15 minutes.
      </output>
    </>
  );
}
