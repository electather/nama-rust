import { PageHeading } from "@hub/app/page-heading";

/** A Sign-in Link's page while the Hub looks the link up, or after it `failed` to. */
export function OpeningSignInLink({ failed }: { failed: boolean }) {
  return (
    <>
      <PageHeading>Sign in</PageHeading>
      {failed ? (
        <p role="alert" className="text-center text-sm text-destructive">
          The Hub couldn’t open this Sign-in Link just now. Reload to try again.
        </p>
      ) : (
        <output className="text-center text-sm text-muted-foreground">
          Opening your Sign-in Link…
        </output>
      )}
    </>
  );
}
