import { useId, useState } from "react";

import { DefaultPermissionsForm } from "@/features/admin-users/components/default-permissions-form";
import type { User } from "@/gen/nama/v1/sign_in_pb";
import { useDefaultPermissions } from "@/shared/hooks/use-default-permissions";
import { labelsOf, mayChangeDefaultPermissions } from "@/shared/lib/permissions";

/** The Permissions new Users get, named, opening to change them when `you` may. */
export function DefaultPermissionsEditor({ you }: { you: User | undefined }) {
  const permissions = useDefaultPermissions();
  const [open, setOpen] = useState(false);
  const panel = useId();
  return (
    <>
      <div className="mt-2 flex items-baseline gap-3">
        <p className="text-sm">{labelsOf(permissions)}</p>
        {mayChangeDefaultPermissions(you, permissions) && (
          <button
            type="button"
            aria-expanded={open}
            aria-controls={panel}
            onClick={() => {
              setOpen(!open);
            }}
            className="ms-auto rounded-sm text-sm text-muted-foreground outline-none hover:text-foreground focus-visible:ring-3 focus-visible:ring-ring/50"
          >
            {open ? "Close" : "Change"}
          </button>
        )}
      </div>
      {open && (
        <div id={panel} className="mt-4">
          <DefaultPermissionsForm initial={permissions} you={you} />
        </div>
      )}
    </>
  );
}
