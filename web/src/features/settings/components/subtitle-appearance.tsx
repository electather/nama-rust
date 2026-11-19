import { useSetSubtitleBackground } from "@/features/settings/data/use-set-subtitle-background";
import { useSetSubtitleSize } from "@/features/settings/data/use-set-subtitle-size";
import { useSubtitleAppearance } from "@/features/settings/data/use-subtitle-appearance";
import {
  backgrounds,
  chosen,
  radios,
  shown,
  sizes,
} from "@/features/settings/lib/subtitle-appearance";
import { Choices } from "@/shared/components/form-field/choices";
import { refusal } from "@/shared/lib/forms";

/** How big the signed-in User's text subtitles are and what's behind them, each saved as it's picked. */
export function SubtitleAppearance() {
  const { size, background } = useSubtitleAppearance();
  const setSize = useSetSubtitleSize();
  const setBackground = useSetSubtitleBackground();
  const failed = setSize.error ?? setBackground.error;
  return (
    <div className="mt-8 flex flex-col gap-5">
      <Choices
        legend="Subtitle size"
        name="subtitleSize"
        choices={radios(sizes)}
        chosen={String(shown(setSize, (saving) => saving.size, size))}
        onChoose={(radio) => {
          setSize.mutate({ size: chosen(sizes, radio) });
        }}
      />
      <Choices
        legend="Subtitle background"
        name="subtitleBackground"
        choices={radios(backgrounds)}
        chosen={String(shown(setBackground, (saving) => saving.background, background))}
        onChoose={(radio) => {
          setBackground.mutate({ background: chosen(backgrounds, radio) });
        }}
      />
      <p className="text-sm text-muted-foreground">
        Styled subtitles keep the look they came with, and burned-in ones can’t change.
      </p>
      {failed && (
        <p role="alert" className="text-sm text-destructive">
          {refusal(failed, "save that", {}).form}
        </p>
      )}
    </div>
  );
}
