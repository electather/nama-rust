import { useEffect } from "react";

import { Approve } from "@/features/auth/components/approve";
import { LookUp } from "@/features/auth/components/look-up";
import { useLookUpSignInCode } from "@/features/auth/data/use-look-up-sign-in-code";
import { Page } from "@/shared/components/page";

/** Signing in another Device: look up the Sign-in Code it shows, check its name, then approve. */
export function EnterCode({
  scanned,
  onStartOver,
}: {
  scanned: string | undefined;
  /** Clears the scanned code from the page's address. */
  onStartOver: () => void;
}) {
  const lookUp = useLookUpSignInCode();
  // `mutate` keeps its identity across renders, unlike `lookUp`, so a re-render doesn't look the scanned code up again.
  const { mutate } = lookUp;
  useEffect(() => {
    if (scanned !== undefined) {
      mutate({ code: scanned });
    }
  }, [scanned, mutate]);
  const startOver = () => {
    lookUp.reset();
    onStartOver();
  };
  return (
    <Page heading="Sign in a Device">
      <section className="mt-2 max-w-sm">
        {lookUp.data === undefined ? (
          <LookUp
            scanned={scanned}
            onSubmit={(code) => {
              lookUp.mutate({ code });
            }}
            pending={lookUp.isPending}
            error={lookUp.error}
          />
        ) : (
          <Approve
            code={lookUp.variables.code ?? ""}
            deviceName={lookUp.data.deviceName}
            onDone={startOver}
          />
        )}
      </section>
    </Page>
  );
}
