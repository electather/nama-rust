import { PaneFrame } from "@/features/settings/components/pane-frame";
import { PlaybackRows } from "@/features/settings/components/playback-rows";
import { SaveToast } from "@/features/settings/components/save-toast";
import { useAutoSave } from "@/features/settings/hooks/use-auto-save";
import { useSavePlayback } from "@/features/settings/hooks/use-save-playback";
import { useSavedPlayback } from "@/features/settings/hooks/use-saved-playback";
import { playbackLabels } from "@/features/settings/lib/playback-values";
import { refusedWords } from "@/features/settings/lib/refused-words";

/**
 * The signed-in User's own Playback settings: the audio and subtitles everything they play starts
 * with, unless they picked others in that Show or Movie (ADR 0035), their Subtitle Appearance, and
 * whether the next Episode plays by itself. Each is saved as it's changed.
 */
export function PlaybackPane() {
  const { values, refused, edit, dismiss } = useAutoSave(useSavedPlayback(), useSavePlayback());
  return (
    <>
      <PaneFrame section="playback">
        <PlaybackRows values={values} onEdit={edit} />
      </PaneFrame>
      <SaveToast says={refusedWords(refused, playbackLabels).form} onDismiss={dismiss} />
    </>
  );
}
