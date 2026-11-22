import { usePlaybackSettings } from "@/features/admin-playback/data/use-playback-settings";
import { usePlaybackStatus } from "@/features/admin-playback/data/use-playback-status";
import { useUpdatePlaybackSettings } from "@/features/admin-playback/data/use-update-playback-settings";
import {
  playbackLabels,
  playbackRefusals,
  playbackSettings,
  savedPlayback,
} from "@/features/admin-playback/lib/playback-settings-form";
import { PaneFrame } from "@/features/settings/components/pane-frame";
import { SaveToast } from "@/features/settings/components/save-toast";
import { ServerPlaybackRows } from "@/features/settings/components/server-playback-rows";
import { useAutoSave } from "@/features/settings/hooks/use-auto-save";
import { refusedWords } from "@/features/settings/lib/refused-words";

/**
 * How the Server plays what it has to change, for an Admin: the ffmpeg and hardware it found to
 * Remux and Transcode with, and its Playback settings, each saved as it's changed.
 */
export function ServerPlaybackPane() {
  const status = usePlaybackStatus();
  const { mutateAsync } = useUpdatePlaybackSettings();
  const { values, refused, edit, dismiss } = useAutoSave(
    savedPlayback(usePlaybackSettings(), status.hardware),
    // Saving replaces every Playback setting, so each save sends what's saved with this change
    // over it: a refused change goes back, and isn't sent again with the next one.
    async (wanted) => {
      await mutateAsync({ settings: playbackSettings(wanted) });
    },
  );
  const errors = refusedWords(
    refused,
    playbackLabels,
    refused && playbackRefusals(refused.failure),
  );
  return (
    <>
      <PaneFrame section="server-playback">
        <ServerPlaybackRows status={status} values={values} errors={errors} onEdit={edit} />
      </PaneFrame>
      <SaveToast says={errors.form} onDismiss={dismiss} />
    </>
  );
}
