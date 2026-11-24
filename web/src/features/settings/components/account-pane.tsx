import { DeleteAccount } from "@/features/settings/components/delete-account";
import { PaneFrame } from "@/features/settings/components/pane-frame";
import { SettingGroup } from "@/features/settings/components/setting-group";
import { useHubAccount } from "@/features/settings/data/use-hub-account";
import { accountLabels } from "@/features/settings/lib/pane-labels";
import { SettingRow } from "@/shared/components/setting-row";
import { rowText } from "@/shared/lib/setting-text";

/** The Account signed in at the Hub: its email, and deleting it. */
export function AccountPane() {
  const { email } = useHubAccount();
  const emailRow = (
    <SettingRow {...rowText(accountLabels, "email")}>
      {(control) => (
        <output {...control} className="text-sm font-medium break-all">
          {email}
        </output>
      )}
    </SettingRow>
  );
  return (
    <PaneFrame section="account">
      {/* Last in its own box, so the row gives back its bottom padding before "Delete Account". */}
      <div>{emailRow}</div>
      <SettingGroup setting="delete" title={accountLabels.delete.label}>
        <DeleteAccount />
      </SettingGroup>
    </PaneFrame>
  );
}
