// fallow-ignore-next-line policy-violation:feature-layout/data-hooks -- no query is built here: a picked Server's Settings run against its transport, as `_member/_server` hands a Server's pages theirs
import { TransportProvider, useTransport } from "@connectrpc/connect-query";
import { useSearch } from "@tanstack/react-router";

import { ServerPicker } from "@/features/settings/components/server-picker";
import { SettingsBody } from "@/features/settings/components/settings-body";
import { useAdminServers } from "@/features/settings/data/use-admin-servers";
import { type Section, groupsFor } from "@/features/settings/lib/sections";

/**
 * Settings at the Hub: the Account's own, or, once `?manage=` picks one of the Servers its User is
 * an Admin on, that Server's Sections, run against it (ADR 0105).
 */
export function HubSettings({
  section,
  header,
}: {
  section: Section;
  header: { titleId: string; onClose: () => void };
}) {
  const hub = useTransport();
  const servers = useAdminServers();
  const manage = useSearch({ strict: false, select: (search) => search.manage });
  // shortcut: a picked Server not reached yet, as on reload, shows the Account's until it is;
  // wait for it if that flash bothers anyone.
  const picked = servers.find(({ server }) => server.id === manage);
  const user = picked?.signedIn.user;
  const picker = <ServerPicker servers={servers} picked={picked} />;
  return (
    // There for the Account's too, so picking a Server keeps the picker it was picked in.
    <TransportProvider transport={picked?.transport ?? hub}>
      <SettingsBody
        groups={groupsFor(user, picked === undefined)}
        section={section}
        user={user}
        header={header}
        picker={picker}
        picked={picked}
      />
    </TransportProvider>
  );
}
