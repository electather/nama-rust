import { createPortal } from "react-dom";

import { hubRefusalReason } from "@/features/auth/lib/hub-errors";
import { SaveToast } from "@/features/settings/components/save-toast";
import { RefusalReason } from "@/gen/nama/hub/v1/refusal_pb";

/** Says the user menu's Sign out failed, until it's dismissed or signing out is tried again. */
export function SignOutToast({
  error,
  atHub,
  onDismiss,
}: {
  error: Error | null;
  atHub: boolean;
  onDismiss: () => void;
}) {
  if (error === null || opensSignIn(error, atHub)) {
    return null;
  }
  // Out of the top bar, whose backdrop blur would hold a fixed toast inside it once it lifts.
  return createPortal(
    <div className="fixed inset-x-0 bottom-0 z-50 flex flex-col">
      <SaveToast
        label="Not signed out"
        says={`The ${atHub ? "Hub" : "Server"} couldn’t sign you out just now. Try again.`}
        onDismiss={onDismiss}
      />
    </div>,
    document.body,
  );
}

/** Whether the Hub refused for want of sign-in, which opens sign-in instead of saying anything. */
function opensSignIn(error: Error, atHub: boolean) {
  return atHub && hubRefusalReason(error) === RefusalReason.SIGN_IN_REQUIRED;
}
