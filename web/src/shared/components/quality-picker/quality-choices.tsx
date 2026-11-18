import type { Quality } from "@/gen/nama/v1/plugin_pb";
import { QualityChoice } from "@/shared/components/quality-picker/quality-choice";

/** `qualities` as radio buttons, the User's `personalDefault` picked, else the default. */
export function QualityChoices({
  qualities,
  personalDefault,
}: {
  qualities: Quality[];
  personalDefault: bigint | undefined;
}) {
  return (
    <fieldset>
      {/* The dialog's description already asks for a Quality, so only a screen reader needs it. */}
      <legend className="sr-only">Quality</legend>
      <ul className="flex flex-col">
        {qualities.map((quality) => (
          <QualityChoice
            key={String(quality.id)}
            quality={quality}
            picked={
              personalDefault === undefined ? quality.isDefault : quality.id === personalDefault
            }
          />
        ))}
      </ul>
    </fieldset>
  );
}
