import { PaneFrame } from "@/features/settings/components/pane-frame";
import { PasswordForm } from "@/features/settings/components/password-form";
import { SectionBoundary } from "@/features/settings/components/section-boundary";
import { SettingGroup } from "@/features/settings/components/setting-group";
import { UserLink } from "@/features/settings/components/user-link";
import { signinLabels } from "@/features/settings/lib/pane-labels";
import type { User } from "@/gen/nama/v1/sign_in_pb";
import { SettingRow } from "@/shared/components/setting-row";
import { rowText } from "@/shared/lib/setting-text";

/**
 * How the signed-in `user` signs in to this Server: their username, changing their password, and
 * their Link to an Account on the Hub.
 */
export function SigninPane({ user }: { user: User | undefined }) {
  const usernameRow = (
    <SettingRow {...rowText(signinLabels, "username")}>
      {(control) => (
        <output {...control} className="text-sm font-medium">
          {user?.name}
        </output>
      )}
    </SettingRow>
  );
  return (
    <PaneFrame section="signin">
      {/* Last in its own box, so the row gives back its bottom padding before "Password". */}
      <div>{usernameRow}</div>
      <SettingGroup title="Password">
        <PasswordForm />
      </SettingGroup>
      {/* Shows once it has loaded, the rest without waiting for it. */}
      <SectionBoundary what="your Account Link">
        <UserLink />
      </SectionBoundary>
    </PaneFrame>
  );
}
