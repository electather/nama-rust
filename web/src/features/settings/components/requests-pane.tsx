import { PaneFrame } from "@/features/settings/components/pane-frame";
import { QualityRows } from "@/features/settings/components/quality-rows";
import { SaveToast } from "@/features/settings/components/save-toast";
import { useDefaultQualities } from "@/features/settings/data/use-default-qualities";
import { useSaveDefaultQuality } from "@/features/settings/data/use-save-default-quality";
import { useAutoSave } from "@/features/settings/hooks/use-auto-save";
import { qualityLabel, savedQualities } from "@/features/settings/lib/default-quality";
import { refusedWords } from "@/features/settings/lib/refused-words";

/**
 * The Quality the signed-in User's Requests ask each Fulfiller for when they don't pick one, for a
 * User who may choose a Quality. Each is saved as it's picked.
 */
export function RequestsPane() {
  const defaults = useDefaultQualities();
  const { mutateAsync } = useSaveDefaultQuality();
  const { values, refused, edit, dismiss } = useAutoSave(
    savedQualities(defaults),
    async (wanted, changed) => {
      await Promise.all(
        changed.map(async (pluginId) => {
          await mutateAsync({ pluginId, quality: BigInt(wanted[pluginId] ?? 0) });
        }),
      );
    },
  );
  const labels = Object.fromEntries(
    defaults.map((fulfiller) => [fulfiller.pluginId, { label: qualityLabel(fulfiller) }]),
  );
  return (
    <>
      <PaneFrame section="requests">
        <QualityRows defaults={defaults} values={values} onEdit={edit} />
      </PaneFrame>
      <SaveToast says={refusedWords(refused, labels).form} onDismiss={dismiss} />
    </>
  );
}
