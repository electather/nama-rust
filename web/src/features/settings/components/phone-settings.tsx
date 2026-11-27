import type { ReactNode } from "react";

import { SettingsHeader } from "@/features/settings/components/settings-header";
import type { Section } from "@/shared/lib/settings-address";

/**
 * Settings on a phone, as two pages, as a phone's own settings are: with `listing`, the list of
 * sections in `side`, with Search; else the `pane` of the section `shown`, which starts with the way
 * back to that list.
 */
export function PhoneSettings({
  listing,
  shown,
  side,
  pane,
  header,
}: {
  listing: boolean;
  shown: { section: Section; title: string; served: boolean };
  side: ReactNode;
  pane: ReactNode;
  header: { key: string; titleId: string; onClose: () => void };
}) {
  const { key, ...named } = header;
  return (
    <>
      <title>{listing ? "Settings – Nama" : shown.title}</title>
      <SettingsHeader
        key={key}
        section={shown.section}
        served={shown.served}
        phone={listing ? "list" : "section"}
        {...named}
      />
      {listing ? side : pane}
    </>
  );
}
