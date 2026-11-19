import { ScanToApprove } from "@/features/auth/components/scan-to-approve";

/** The code, with its QR code while it can still be approved. */
export function ShownCode({ code, expired }: { code: string; expired: boolean }) {
  return (
    <div className="rounded-xl border bg-card px-4 py-8 text-center">
      <p
        className={
          expired
            ? "text-4xl font-semibold tracking-wide whitespace-nowrap text-muted-foreground tabular-nums line-through"
            : "text-4xl font-semibold tracking-wide whitespace-nowrap tabular-nums"
        }
      >
        {code}
      </p>
      {!expired && <ScanToApprove code={code} />}
      <output className="mt-3 block text-sm text-muted-foreground">
        {expired ? "This code has expired." : "Waiting for it to be approved…"}
      </output>
    </div>
  );
}
