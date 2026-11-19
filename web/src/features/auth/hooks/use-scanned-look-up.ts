import { useNavigate } from "@tanstack/react-router";
import { useEffect } from "react";

import { useLookUpSignInCode } from "@/features/auth/data/use-look-up-sign-in-code";

/**
 * Looks up Sign-in Codes, starting with `scanned`, the one a QR code opened the page with, and
 * goes on to approving the Device one belongs to. A scanned code's page gives way to it, so going
 * back doesn't look it up again. `code` is the last one looked up, or else the scanned one.
 */
export function useScannedLookUp(scanned: string | undefined) {
  const navigate = useNavigate();
  const lookUp = useLookUpSignInCode(async (code, deviceName) => {
    await navigate({
      to: "/code/approve",
      state: { lookedUp: { code, deviceName } },
      replace: code === scanned,
    });
  });
  // `mutate` keeps its identity across renders, unlike `lookUp`, so a re-render doesn't look the scanned code up again.
  const { mutate } = lookUp;
  useEffect(() => {
    if (scanned !== undefined) {
      mutate({ code: scanned });
    }
  }, [scanned, mutate]);
  return { lookUp, code: lookUp.variables?.code ?? scanned };
}
