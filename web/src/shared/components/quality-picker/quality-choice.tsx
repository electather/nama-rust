import { useId } from "react";

import type { Quality } from "@/gen/nama/v1/plugin_pb";
import { QualityLabel } from "@/shared/components/quality-picker/quality-label";

/** One Quality as a radio button with its description, marked if it's the default. */
export function QualityChoice({ quality, picked }: { quality: Quality; picked: boolean }) {
  const descriptionId = useId();
  const described = quality.description !== "";
  return (
    <li className="flex min-h-12 flex-col justify-center border-b py-2 last:border-b-0">
      <QualityLabel
        quality={quality}
        picked={picked}
        descriptionId={described ? descriptionId : undefined}
      />
      {described && (
        // Lined up with the label's text, past the radio button and the gap after it.
        <p id={descriptionId} className="ps-7 text-sm text-muted-foreground">
          {quality.description}
        </p>
      )}
    </li>
  );
}
