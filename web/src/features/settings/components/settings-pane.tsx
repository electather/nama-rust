import type { ComponentType } from "react";

import { BackupsPane } from "@/features/settings/components/backups-pane";
import { ConnectionsPane } from "@/features/settings/components/connections-pane";
import { DefaultHomePane } from "@/features/settings/components/default-home-pane";
import { DevicesPane } from "@/features/settings/components/devices-pane";
import { HomePane } from "@/features/settings/components/home-pane";
import { PaneBoundary } from "@/features/settings/components/pane-boundary";
import { PlaybackPane } from "@/features/settings/components/playback-pane";
import { PluginsPane } from "@/features/settings/components/plugins-pane";
import { RequestsPane } from "@/features/settings/components/requests-pane";
import { ServerPane } from "@/features/settings/components/server-pane";
import { ServerPlaybackPane } from "@/features/settings/components/server-playback-pane";
import { SigninPane } from "@/features/settings/components/signin-pane";
import { UsersPane } from "@/features/settings/components/users-pane";
import type { Section } from "@/features/settings/lib/sections";
import type { User } from "@/gen/nama/v1/sign_in_pb";

const panes: Record<Section, ComponentType<{ user: User | undefined }>> = {
  signin: SigninPane,
  devices: DevicesPane,
  playback: PlaybackPane,
  home: HomePane,
  requests: RequestsPane,
  connections: ConnectionsPane,
  server: ServerPane,
  users: UsersPane,
  "default-home": DefaultHomePane,
  "server-playback": ServerPlaybackPane,
  backups: BackupsPane,
  plugins: PluginsPane,
};

/** The pane of the `section` open: its title over its settings, which load on their own. */
export function SettingsPane({ section, user }: { section: Section; user: User | undefined }) {
  const Pane = panes[section];
  return (
    // Under what sticks at the top of a pane, a field scrolled to for focus would be hidden.
    // ponytail: one height for whatever sticks, the tallest. Measure it if one outgrows this.
    <div className="flex min-w-0 flex-1 flex-col overflow-y-auto overscroll-contain md:has-data-sticks:scroll-pt-48">
      <PaneBoundary section={section}>
        <Pane user={user} />
      </PaneBoundary>
    </div>
  );
}
