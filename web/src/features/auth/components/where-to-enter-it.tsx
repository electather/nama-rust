/** Where to enter the code this browser shows. */
export function WhereToEnterIt() {
  return (
    <>
      Scan the QR code with your phone. Or, on a Device where you’re signed in, go to{" "}
      <strong className="font-medium text-foreground">{globalThis.location.origin}/code</strong> and
      enter this code.
    </>
  );
}
