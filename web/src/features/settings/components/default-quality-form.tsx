import { QualitySelect } from "@/features/settings/components/quality-select";
import { useSaveDefaultQuality } from "@/features/settings/data/use-save-default-quality";
import { fulfillerLabels } from "@/features/settings/lib/default-quality";
import type { DefaultQuality } from "@/gen/nama/v1/request_pb";
import { FormEnd } from "@/shared/components/form-field/form-end";
import { formText, onSubmitted, refusal } from "@/shared/lib/forms";

/** The form a User sets their default from one Fulfiller's Qualities with. */
export function DefaultQualityForm({ fulfiller }: { fulfiller: DefaultQuality }) {
  const { pluginId } = fulfiller;
  const kind = fulfillerLabels[fulfiller.fulfiller] ?? pluginId;
  const save = useSaveDefaultQuality();
  return (
    <form
      className="mt-4 flex flex-col gap-5"
      onSubmit={onSubmitted((typed) => {
        save.mutate({ pluginId, quality: BigInt(formText(typed, "quality")) });
      })}
    >
      <QualitySelect label={kind} fulfiller={fulfiller} />
      <FormEnd
        action={`Save default for ${kind}`}
        variant="ghost"
        pending={save.isPending}
        done={save.isSuccess ? "Saved." : undefined}
        refusal={
          save.error ? refusal(save.error, `save your default for ${kind}`, {}).form : undefined
        }
      />
    </form>
  );
}
