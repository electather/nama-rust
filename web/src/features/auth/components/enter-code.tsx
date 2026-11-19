import { LookUp } from "@/features/auth/components/look-up";
import { useScannedLookUp } from "@/features/auth/hooks/use-scanned-look-up";
import type { FormErrors } from "@/shared/lib/forms";

/** The first step of signing in another Device: the Sign-in Code it shows. */
export function EnterCode({
  scanned,
  refused,
}: {
  scanned: string | undefined;
  /** A code the Server just refused to approve, kept, with why. */
  refused: { code: string; errors: FormErrors } | undefined;
}) {
  const { lookUp, code = refused?.code } = useScannedLookUp(scanned);
  return (
    <LookUp
      key={code}
      code={code}
      onSubmit={(typed) => {
        lookUp.mutate({ code: typed });
      }}
      pending={lookUp.isPending}
      error={lookUp.error}
      refused={refused?.errors}
    />
  );
}
