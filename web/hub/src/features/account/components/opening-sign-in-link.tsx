/** A Sign-in Link's page while the Hub looks the link up, or after it `failed` to. */
export function OpeningSignInLink({ failed }: { failed: boolean }) {
  return (
    <>
      <h1 className="text-center text-3xl font-medium tracking-tight text-balance">Sign in</h1>
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
