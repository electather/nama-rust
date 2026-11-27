import type { ReactElement, ReactNode } from "react";

import { SignInOnHubDialog } from "@/features/auth/components/sign-in-on-hub-dialog";
import { PhoneSettings } from "@/features/settings/components/phone-settings";
import { SettingsHeader } from "@/features/settings/components/settings-header";
import { SettingsPane } from "@/features/settings/components/settings-pane";
import { SettingsSide } from "@/features/settings/components/settings-side";
import { UpdateServer } from "@/features/settings/components/update-server";
import type { AdminServer } from "@/features/settings/data/use-admin-servers";
import { usePhoneWidth } from "@/features/settings/hooks/use-phone-width";
import {
  type Section,
  type SectionGroup,
  type SettingsPage,
  allSections,
  openSection,
  sectionTitles,
} from "@/features/settings/lib/sections";
import type { User } from "@/gen/nama/v1/sign_in_pb";
import { ConfirmItsYou } from "@/shared/components/confirm-its-you/confirm-its-you";

/**
 * What Settings holds: the sections in `groups` beside the pane of the one `section` asks for, run
 * as `user`. At the Hub, the `picker` sits over them, and a Section the `picked` Server doesn't
 * serve says to update it. A picked Server's pane has its own "Confirm it’s you", which signs in on
 * the Hub again, as its pages there do (`app/AGENTS.md`). On a phone they're two pages, as a
 * phone's settings are: `all` sections, with Search, and a section's pane, without.
 */
export function SettingsBody({
  groups,
  section,
  user,
  header,
  picker,
  picked,
}: {
  groups: SectionGroup[];
  section: SettingsPage;
  user: User | undefined;
  header: { titleId: string; onClose: () => void };
  picker?: ReactNode;
  picked?: AdminServer | undefined;
}) {
  const phone = usePhoneWidth();
  const shown = openSection(groups, section);
  const served = serves(picked, shown);
  const shownPane = paneOf(
    <SettingsPane key={shown} section={shown} user={user} />,
    picked,
    served,
  );
  const title = `Settings: ${sectionTitles[shown]} – Nama`;
  const side = (
    <SettingsSide groups={groups} section={shown} user={user}>
      {picker}
    </SettingsSide>
  );
  // Keyed by the Server like the pane, so a crumb that failed on one Server asks the next.
  const headerKey = `header-${picked?.server.id}`;
  if (phone) {
    return (
      <PhoneSettings
        listing={section === allSections}
        shown={{ section: shown, title, served }}
        side={side}
        pane={shownPane}
        header={{ key: headerKey, ...header }}
      />
    );
  }
  return (
    <>
      {/* Each page's `PageTitle` gives the tab way to this while Settings is open. */}
      <title>{title}</title>
      {side}
      {/* The sections run the full height beside the title and the pane. */}
      <div className="flex min-h-0 min-w-0 flex-1 flex-col">
        <SettingsHeader key={headerKey} section={shown} served={served} {...header} />
        {shownPane}
      </div>
    </>
  );
}

/**
 * `pane` as a `picked` Server shows it: under its own "Confirm it’s you" if it serves the section,
 * else what says to update it. Keyed by the Server, so one Server's pane never shows on another's.
 */
function paneOf(
  pane: ReactElement,
  picked: AdminServer | undefined,
  served: boolean,
): ReactElement {
  if (picked === undefined) {
    return pane;
  }
  return served ? (
    <ConfirmItsYou key={picked.server.id} Dialog={SignInOnHubDialog}>
      {pane}
    </ConfirmItsYou>
  ) : (
    <UpdateServer server={picked.server} />
  );
}

/** Whether a `picked` Server reports serving `section`, as the Account's Settings always do. */
function serves(picked: AdminServer | undefined, section: Section) {
  return picked?.settingsSections.includes(section) !== false;
}
