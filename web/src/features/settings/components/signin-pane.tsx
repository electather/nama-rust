import { PaneFrame } from "@/features/settings/components/pane-frame";
import { PasswordForm } from "@/features/settings/components/password-form";
import { SettingGroup } from "@/features/settings/components/setting-group";
import type { User } from "@/gen/nama/v1/sign_in_pb";
import { SettingRow } from "@/shared/components/setting-row";

/** How the signed-in `user` signs in to this Server: their username, and changing their password. */
export function SigninPane({ user }: { user: User | undefined }) {
  return (
    <PaneFrame section="signin">
      <SettingRow label="Username" hint="You sign in with this.">
        {(control) => (
          <output {...control} className="text-sm font-semibold">
            {user?.name}
          </output>
        )}
      </SettingRow>
      <SettingGroup title="Password">
        <PasswordForm />
      </SettingGroup>
    </PaneFrame>
  );
}
