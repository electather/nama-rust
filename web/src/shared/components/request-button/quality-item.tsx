import { DropdownMenuRadioItem } from "nama-ui/components/dropdown-menu";
import { useId } from "react";

import type { Quality } from "@/gen/nama/v1/plugin_pb";

/**
 * One Quality in the Quality menu, marked if it's the Fulfiller's default, with its description,
 * which Requests at it when picked.
 */
export function QualityItem({
  quality,
  onPick,
}: {
  quality: Quality;
  onPick: (quality: bigint) => void;
}) {
  const nameId = useId();
  const descriptionId = useId();
  const described = quality.description !== "";
  return (
    <DropdownMenuRadioItem
      value={quality.id}
      closeOnClick
      aria-labelledby={nameId}
      aria-describedby={described ? descriptionId : undefined}
      // On click, not on the group's change, so picking the checked one Requests at it too.
      onClick={() => {
        onPick(quality.id);
      }}
    >
      <span className="flex flex-col py-1.5">
        <span id={nameId}>{quality.isDefault ? `${quality.name} (Default)` : quality.name}</span>
        {described && (
          <span id={descriptionId} className="text-muted-foreground">
            {quality.description}
          </span>
        )}
      </span>
    </DropdownMenuRadioItem>
  );
}
