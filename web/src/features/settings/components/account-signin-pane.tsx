import { AccountPasswordForm } from "@/features/settings/components/account-password-form";
import { AddPasskeyForm } from "@/features/settings/components/add-passkey-form";
import { PaneFrame } from "@/features/settings/components/pane-frame";
import { PasskeyItems } from "@/features/settings/components/passkey-items";
import { SettingGroup } from "@/features/settings/components/setting-group";
import { accountSigninLabels } from "@/features/settings/lib/pane-labels";

/** How the Account signed in at the Hub signs in: its password and its passkeys. */
export function AccountSigninPane() {
  return (
    <PaneFrame section="account-signin">
      <SettingGroup setting="password" title={accountSigninLabels.password.label}>
        <AccountPasswordForm />
      </SettingGroup>
      <SettingGroup setting="passkeys" title={accountSigninLabels.passkeys.label}>
        <PasskeyItems />
        <AddPasskeyForm />
      </SettingGroup>
    </PaneFrame>
  );
}
