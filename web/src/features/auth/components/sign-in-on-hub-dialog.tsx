import { useLocation } from "@tanstack/react-router";
import type { RefObject } from "react";

import { useSignOutAccount } from "@/features/auth/data/use-sign-out-account";
import { ConfirmModal } from "@/shared/components/confirm-dialog/confirm-modal";

/**
 * "Confirm it’s you" at the Hub, for a change on one of the Account's Servers. The Device the page
 * holds there has Recent Authentication only from a Hub sign-in in the last few minutes, so this
 * signs in on the Hub again, coming back here to make the change again (ADR 0105).
 */
export function SignInOnHubDialog({
  dialog,
  openings,
  confirming,
  onCancel,
  onClose,
}: {
  dialog: RefObject<HTMLDialogElement | null>;
  openings: number;
  /** What it's for, such as `sign in “Laptop”`, or else it's to go on with a change. */
  confirming: string | undefined;
  onCancel: () => void;
  onClose: () => void;
}) {
  const here = useLocation({ select: (location) => location.href });
  // It signs out, since the sign-in page sends a signed-in Account on.
  const signOut = useSignOutAccount(here);
  return (
    <ConfirmModal
      ref={dialog}
      openings={openings}
      onCancel={onCancel}
      onClose={onClose}
      action="Sign in again"
      title="Confirm it’s you"
      description={`Sign in to the Hub again to ${confirming ?? "go on"}. This keeps anyone else using this browser from doing it.`}
      onConfirm={async () => {
        await signOut.mutateAsync({});
      }}
      refusal={() => "The Hub couldn’t sign you out just now. Try again."}
    />
  );
}
