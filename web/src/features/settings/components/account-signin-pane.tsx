import { AccountPasswordForm } from "@/features/settings/components/account-password-form";
import { PaneFrame } from "@/features/settings/components/pane-frame";
import { PasskeysGroup } from "@/features/settings/components/passkeys-group";
import { SettingGroup } from "@/features/settings/components/setting-group";
import { accountSigninLabels } from "@/features/settings/lib/pane-labels";

/** How the Account signed in at the Hub signs in: its password and its passkeys. */
export function AccountSigninPane() {
  return (
    <PaneFrame section="account-signin">
      <SettingGroup setting="password" title={accountSigninLabels.password.label}>
        <AccountPasswordForm />
      </SettingGroup>
      <PasskeysGroup />
    </PaneFrame>
  );
}
