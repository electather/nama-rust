import { useEffect, useId, useRef } from "react";

import { PermissionsEditor } from "@/features/admin-users/components/permissions-editor";
import { UserActions } from "@/features/admin-users/components/user-actions";
import { UserDevices } from "@/features/admin-users/components/user-devices";
import { UserCrumbs } from "@/features/settings/components/user-crumbs";
import type { User } from "@/gen/nama/v1/sign_in_pb";
import { labelsOf } from "@/shared/lib/permissions";

/**
 * The User `user` in place of the list, under the crumbs back to it: resetting their password and
 * deleting them at the end of their name, then their Permissions and their Devices.
 */
export function UserDetail({ user, you }: { user: User; you: User | undefined }) {
  const titleId = useId();
  const title = useRef<HTMLHeadingElement>(null);
  useEffect(() => {
    // The list this replaced held focus, so it moves to what took its place.
    title.current?.focus();
  }, []);
  const name = (
    <div className="flex min-w-0 flex-1 basis-50 flex-col gap-0.5">
      <h3
        ref={title}
        id={titleId}
        tabIndex={-1}
        className="truncate text-2xl font-semibold tracking-tight outline-none"
      >
        {user.name}
      </h3>
      <p className="text-sm text-muted-foreground">{labelsOf(user.permissions)}</p>
    </div>
  );
  const header = (
    <header className="px-4 pt-4 md:px-7.5 md:pt-5">
      <UserCrumbs name={user.name} />
      <div className="mt-3.5 flex flex-wrap items-center gap-x-4 gap-y-2.5 border-b pb-6">
        {name}
        <UserActions user={user} isYou={user.id === you?.id} />
      </div>
    </header>
  );
  return (
    <section aria-labelledby={titleId} className="flex flex-col">
      {header}
      <div className="flex flex-col gap-7.5 px-4 pt-6 pb-4 md:px-7.5 md:pb-7.5">
        <PermissionsEditor user={user} you={you} />
        <UserDevices user={user} />
      </div>
    </section>
  );
}
