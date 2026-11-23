import { PaneSection } from "@/features/settings/components/pane-section";
import { QualityRows } from "@/features/settings/components/quality-rows";
import { SaveToast } from "@/features/settings/components/save-toast";
import { useDefaultQualities } from "@/features/settings/data/use-default-qualities";
import { useSaveDefaultQuality } from "@/features/settings/data/use-save-default-quality";
import { useAutoSave } from "@/features/settings/hooks/use-auto-save";
import { qualityLabel, savedQualities } from "@/features/settings/lib/default-quality";
import { refusedWords } from "@/features/settings/lib/refused-words";

/**
 * The Quality the signed-in User's Requests ask each Fulfiller for when they don't pick one. Each
 * is saved as it's picked. Nothing when every Fulfiller offers only one.
 */
export function DefaultQualities() {
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
  if (defaults.length === 0) {
    return null;
  }
  return (
    <PaneSection title="Default Quality">
      <QualityRows defaults={defaults} values={values} onEdit={edit} />
      <SaveToast says={refusedWords(refused, labels).form} onDismiss={dismiss} />
    </PaneSection>
  );
}
