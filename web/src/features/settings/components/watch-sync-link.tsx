import { CardHeading } from "@/features/settings/components/card-heading";
import { WatchSyncLinkState } from "@/features/settings/components/watch-sync-link-state";
import { useDeletePersonalSettings } from "@/features/settings/data/use-delete-personal-settings";
import { useStartWatchSyncLink } from "@/features/settings/data/use-start-watch-sync-link";
import { useWatchSyncLinkPoll } from "@/features/settings/data/use-watch-sync-link-poll";
import { linkFailed, linkView } from "@/features/settings/lib/watch-sync-link";
import type { PersonalSettings as Personal } from "@/gen/nama/v1/user_pb";

/**
 * Linking the signed-in User's account at a Watch sync Plugin's tracker, such as Trakt, by a code
 * they enter there, under the Plugin's name as the heading `headingId`. The Server keeps the
 * tokens as their secret Personal settings, so the account is linked while any is set.
 */
export function WatchSyncLink({ plugin, headingId }: { plugin: Personal; headingId: string }) {
  const start = useStartWatchSyncLink();
  const poll = useWatchSyncLinkPoll(plugin.pluginId, start.data);
  const unlink = useDeletePersonalSettings();
  const linked = plugin.settings.some((setting) => setting.secretSet);
  const failed = linkFailed(start.error ?? poll.error, unlink.error, plugin.name);
  return (
    <div className="flex flex-col gap-3">
      <CardHeading plugin={plugin} id={headingId} />
      <WatchSyncLinkState
        view={linkView(start.data !== undefined, poll.state, linked)}
        started={start.data}
        tracker={plugin.name}
        pending={start.isPending || unlink.isPending}
        onStart={() => {
          unlink.reset();
          start.mutate({ pluginId: plugin.pluginId });
        }}
        onCancel={() => {
          start.reset();
        }}
        onUnlink={() => {
          start.reset();
          unlink.mutate({ pluginId: plugin.pluginId });
        }}
      />
      <p role="alert" className="text-sm text-destructive empty:hidden">
        {failed}
      </p>
    </div>
  );
}
