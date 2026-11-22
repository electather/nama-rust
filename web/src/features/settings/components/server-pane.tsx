import { ImageStore } from "@/features/settings/components/image-store";
import { PaneFrame } from "@/features/settings/components/pane-frame";
import { SaveToast } from "@/features/settings/components/save-toast";
import { ServerLink } from "@/features/settings/components/server-link";
import { ServerRows } from "@/features/settings/components/server-rows";
import { SharingRows } from "@/features/settings/components/sharing-rows";
import { useImageSettings } from "@/features/settings/data/use-image-settings";
import { useServerSettings } from "@/features/settings/data/use-server-settings";
import { useAutoSave } from "@/features/settings/hooks/use-auto-save";
import { useSaveServer } from "@/features/settings/hooks/use-save-server";
import { serverRefusal } from "@/features/settings/lib/server-refusal";
import { savedServer } from "@/features/settings/lib/server-values";

/**
 * What a User who manages settings sets for everyone on the Server: its address, how Posters and
 * Backdrops are resized, where its images are kept, its Link to the Hub, and what it shares with
 * Nama's developers. All but the image store and the Link are saved as they're changed; switching
 * the image store and Linking are jobs of their own.
 */
export function ServerPane() {
  const saved = savedServer(useServerSettings().settings, useImageSettings().settings);
  const { values, refused, edit, dismiss } = useAutoSave(saved, useSaveServer());
  const words = serverRefusal(refused);
  return (
    <>
      <PaneFrame section="server">
        <ServerRows values={values} urlRefused={words.serverUrl} onEdit={edit} />
        <ImageStore />
        <ServerLink />
        <SharingRows values={values} onEdit={edit} />
      </PaneFrame>
      <SaveToast says={words.form} onDismiss={dismiss} />
    </>
  );
}
