import { AddPasskeyForm } from "@/features/settings/components/add-passkey-form";
import { PasskeyItems } from "@/features/settings/components/passkey-items";
import { SectionBoundary } from "@/features/settings/components/section-boundary";
import { SettingGroup } from "@/features/settings/components/setting-group";
import { accountSigninLabels } from "@/features/settings/lib/pane-labels";

/**
 * The Account's passkeys and adding one, which load on their own, so the password form over them
 * works without the list.
 */
export function PasskeysGroup() {
  return (
    <SettingGroup setting="passkeys" title={accountSigninLabels.passkeys.label}>
      <SectionBoundary what="your passkeys">
        <PasskeyItems />
        <AddPasskeyForm />
      </SectionBoundary>
    </SettingGroup>
  );
}
