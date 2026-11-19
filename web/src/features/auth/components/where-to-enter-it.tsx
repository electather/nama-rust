/** The steps that approve the code this browser shows, on a Device already signed in. */
export function WhereToEnterIt({ deviceName }: { deviceName: string }) {
  return (
    <ol className="flex list-decimal flex-col gap-3.5 border-t ps-5 pt-5 text-sm marker:text-muted-foreground marker:tabular-nums">
      <li className="ps-1">
        On a Device where you’re signed in, open{" "}
        <strong className="font-semibold">{globalThis.location.host}/code</strong>, or scan the QR
        code with your phone.
      </li>
      <li className="ps-1">Enter this code.</li>
      <li className="ps-1">
        Check that it says <strong className="font-semibold wrap-anywhere">{deviceName}</strong>,
        then sign it in. You may be asked for your password there.
      </li>
    </ol>
  );
}
