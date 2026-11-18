import { useSuspenseQuery } from "@connectrpc/connect-query";
import { useId, useState } from "react";

import { CreateInvite } from "@/components/create-invite";
import { InviteRow } from "@/components/invite-row";
import type { User } from "@/gen/nama/v1/sign_in_pb";
import { UserService } from "@/gen/nama/v1/user_pb";
import { AdminList } from "@/shared/components/admin-list/admin-list";

/** The pending Invites, and a form to make one while the Server has a Server URL to link to. */
export function Invites({ you }: { you: User | undefined }) {
  const { data } = useSuspenseQuery(UserService.method.listInvites, {});
  return (
    <>
      {data.serverUrlSet ? (
        <MakeAnInvite you={you} />
      ) : (
        <p className="mt-4 text-sm text-muted-foreground">
          An Invite’s link needs this Server’s Server URL, which is set in Settings.
        </p>
      )}
      {data.invites.length > 0 && (
        <AdminList>
          {data.invites.map((invite) => (
            <InviteRow key={invite.id} invite={invite} />
          ))}
        </AdminList>
      )}
    </>
  );
}

/** A button opening the form that makes an Invite, as the Default Permissions' "Change" does. */
function MakeAnInvite({ you }: { you: User | undefined }) {
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
