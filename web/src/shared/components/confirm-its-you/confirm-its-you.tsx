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
  const confirmed = useRef(false);

  const ask = useCallback(async () => {
    // Changes refused while the dialog is already open wait for the same answer.
    if (!answer.current) {
      answer.current = new Promise<boolean>((resolve) => {
        give.current = resolve;
      });
      confirmed.current = false;
      setOpenings((each) => each + 1);
      dialog.current?.showModal();
    }
    const isConfirmed = await answer.current;
    return isConfirmed;
  }, []);

  return (
    <AskContext value={ask}>
      {children}
      <ReauthenticateDialog
        dialog={dialog}
        openings={openings}
        onReauthenticated={() => {
          confirmed.current = true;
        }}
        onClose={() => {
          give.current?.(confirmed.current);
          answer.current = undefined;
        }}
      />
    </AskContext>
  );
}
