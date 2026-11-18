import { useId, useState } from "react";

import { CreateInvite } from "@/features/invites/components/create-invite";
import type { User } from "@/gen/nama/v1/sign_in_pb";

/** A button opening the form that makes an Invite, as the Default Permissions' "Change" does. */
export function MakeAnInvite({ you }: { you: User | undefined }) {
  const [open, setOpen] = useState(false);
  const panel = useId();
  return (
    <>
      <button
        type="button"
        aria-expanded={open}
        aria-controls={panel}
        onClick={() => {
          setOpen(!open);
        }}
        className="mt-3 rounded-sm text-sm text-muted-foreground outline-none hover:text-foreground focus-visible:ring-3 focus-visible:ring-ring/50"
      >
        {open ? "Close" : "Make an Invite"}
      </button>
      {open && (
        <div id={panel}>
          <CreateInvite you={you} />
        </div>
      )}
    </>
  );
}
