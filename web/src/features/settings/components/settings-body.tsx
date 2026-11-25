import type { ReactNode } from "react";

import { SettingsHeader } from "@/features/settings/components/settings-header";
import { SettingsPane } from "@/features/settings/components/settings-pane";
import { SettingsSide } from "@/features/settings/components/settings-side";
import { UpdateServer } from "@/features/settings/components/update-server";
import type { AdminServer } from "@/features/settings/data/use-admin-servers";
import {
  type Section,
  type SectionGroup,
  openSection,
  sectionTitles,
} from "@/features/settings/lib/sections";
import type { User } from "@/gen/nama/v1/sign_in_pb";
import { ConfirmItsYou } from "@/shared/components/confirm-its-you/confirm-its-you";

/**
 * What Settings holds: the sections in `groups` beside the pane of the one `section` asks for, run
 * as `user`. At the Hub, the `picker` sits over them, and a Section the `picked` Server doesn't
 * serve says to update it. A picked Server's pane has its own "Confirm it’s you", as its pages there
 * do (`app/AGENTS.md`).
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
  section: Section;
  user: User | undefined;
  header: { titleId: string; onClose: () => void };
  picker?: ReactNode;
  picked?: AdminServer | undefined;
}) {
  const shown = openSection(groups, section);
  const pane = <SettingsPane key={shown} section={shown} user={user} />;
  // Keyed by the Server, so what one Server's pane holds never shows on another's.
  const pickedPane =
    picked &&
    (picked.settingsSections.includes(shown) ? (
      <ConfirmItsYou key={picked.server.id}>{pane}</ConfirmItsYou>
    ) : (
      <UpdateServer server={picked.server} />
    ));
  return (
    <>
      {/* Each page's `PageTitle` gives the tab way to this while Settings is open. */}
      <title>{`Settings: ${sectionTitles[shown]} – Nama`}</title>
      <SettingsSide groups={groups} section={shown} user={user}>
        {picker}
      </SettingsSide>
      {/* Wider, the sections run the full height beside the title and the pane. A phone drops
          this box so its title can sit over the strip of sections. */}
      <div className="flex min-h-0 min-w-0 flex-1 flex-col max-md:contents">
        <SettingsHeader section={shown} {...header} />
        {pickedPane ?? pane}
      </div>
    </>
  );
}
