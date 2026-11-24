import { type ReactNode, useCallback, useRef, useState } from "react";

import { ReauthenticateDialog } from "@/shared/components/confirm-its-you/reauthenticate-dialog";
import { AskContext } from "@/shared/hooks/use-sensitive-mutation";

/**
 * Holds the one "Confirm it’s you" dialog that {@link useSensitiveMutation} opens, for the pages
 * inside it.
 */
export function ConfirmItsYou({ children }: { children: ReactNode }) {
  const dialog = useRef<HTMLDialogElement>(null);
  const [openings, setOpenings] = useState(0);
  const answer = useRef<Promise<boolean>>(undefined);
  const give = useRef<(confirmed: boolean) => void>(undefined);
  const [confirming, setConfirming] = useState<string>();

  const ask = useCallback(async (what: string | undefined) => {
    // Changes refused while the dialog is already open wait for the same answer.
    if (!answer.current) {
      answer.current = new Promise<boolean>((resolve) => {
        give.current = resolve;
      });
      setConfirming(what);
      setOpenings((each) => each + 1);
      dialog.current?.showModal();
    }
    const isConfirmed = await answer.current;
    return isConfirmed;
  }, []);

  function answerWith(confirmed: boolean) {
    give.current?.(confirmed);
    answer.current = undefined;
  }

  return (
    <AskContext value={ask}>
      {children}
      <ReauthenticateDialog
        dialog={dialog}
        openings={openings}
        confirming={confirming}
        // Answered here, not as it closes: Chromium fires `close` only when it next draws the page,
        // which a hidden window may not do for a long time.
        onReauthenticated={() => {
          answerWith(true);
        }}
        onClose={() => {
          // A late `close` may arrive after the next "Confirm it’s you" opened, which it isn't for.
          if (dialog.current?.open !== true) {
            answerWith(false);
          }
        }}
      />
    </AskContext>
  );
}
