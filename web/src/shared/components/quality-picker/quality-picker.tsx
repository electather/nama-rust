import type { RefObject } from "react";

import type { Quality } from "@/gen/nama/v1/plugin_pb";
import { ConfirmModal } from "@/shared/components/confirm-dialog/confirm-modal";
import { QualityChoices } from "@/shared/components/quality-picker/quality-choices";
import { formText } from "@/shared/lib/forms";

/**
 * A dialog that asks which of `qualities` to Request `title` at, starting at the User's
 * `personalDefault` if they have one, else the default, then hands its id to `onPick`, which
 * rejects if the Server refuses. Nothing while there are no `qualities`. Open it with
 * `showModal()` on `ref`, bumping `openings` first.
 */
export function QualityPicker({
  ref,
  openings,
  title,
  qualities,
  personalDefault,
  onPick,
  refusal,
}: {
  ref: RefObject<HTMLDialogElement | null>;
  openings: number;
  title: string;
  qualities: Quality[];
  personalDefault: bigint | undefined;
  onPick: (quality: bigint) => Promise<unknown>;
  refusal: (error: unknown) => string | undefined;
}) {
  if (qualities.length === 0) {
    return null;
  }
  return (
    <ConfirmModal
      ref={ref}
      openings={openings}
      action="Request"
      title={`Request ${title}`}
      description="Pick the Quality to download at."
      onConfirm={async (typed) => {
        await onPick(BigInt(formText(typed, "quality")));
      }}
      refusal={refusal}
    >
      <QualityChoices qualities={qualities} personalDefault={personalDefault} />
    </ConfirmModal>
  );
}
