import { useId } from "react";

import { type PickKind, pickKinds } from "@/features/admin-jobs/lib/schedule-picks";
import { Segmented } from "@/features/settings/components/segmented";

/** How often a schedule runs, as choices in one pill. */
export function KindChoices({
  value,
  onChange,
}: {
  value: PickKind;
  onChange: (kind: PickKind) => void;
}) {
  const id = useId();
  return (
    <div>
      <span id={id} className="sr-only">
        How often
      </span>
      <Segmented
        aria-labelledby={id}
        name="schedule-kind"
        options={pickKinds}
        value={value}
        onChange={onChange}
      />
    </div>
  );
}
