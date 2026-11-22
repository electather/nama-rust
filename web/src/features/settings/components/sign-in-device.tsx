import { useState } from "react";

import { codeErrors } from "@/features/auth/lib/sign-in-errors";
import { DeviceApprove } from "@/features/settings/components/device-approve";
import { DeviceCode } from "@/features/settings/components/device-code";
import type { FormErrors } from "@/shared/lib/forms";

type Step =
  | { step: "code"; code?: string; refused?: FormErrors }
  | { step: "approve"; code: string; deviceName: string };

/**
 * Signing in another Device from the Devices pane, as `/code` does on a page: its Sign-in Code,
 * then which Device it is. Approving it ends with `onSignedIn`; cancelling, with `onCancel`.
 * A refused approval goes back to the code, saying why.
 */
export function SignInDevice({
  onSignedIn,
  onCancel,
}: {
  onSignedIn: (deviceName: string) => void;
  onCancel: () => void;
}) {
  const [at, setAt] = useState<Step>({ step: "code" });
  if (at.step === "code") {
    return (
      <DeviceCode
        code={at.code}
        refused={at.refused}
        onFound={(code, deviceName) => {
          setAt({ step: "approve", code, deviceName });
        }}
        onCancel={onCancel}
      />
    );
  }
  return (
    <DeviceApprove
      code={at.code}
      deviceName={at.deviceName}
      onApproved={() => {
        onSignedIn(at.deviceName);
      }}
      onRefused={(error) => {
        setAt({ step: "code", code: at.code, refused: codeErrors(error) });
      }}
      onCancel={onCancel}
    />
  );
}
