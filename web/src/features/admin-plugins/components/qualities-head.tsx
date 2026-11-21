import { Plus } from "lucide-react";
import type { Ref } from "react";

import { Button } from "@/shared/ui/button";

/**
 * The Qualities heading, with the button that adds one at its end where `onAdd` is given. The
 * button, which `ref` reaches, is pulled into the corner of the row, so it doesn't make it taller.
 */
export function QualitiesHead({
  ref,
  onAdd,
}: {
  ref?: Ref<HTMLButtonElement>;
  onAdd?: (() => void) | undefined;
}) {
  const add = onAdd && (
    <span className="-my-2 -me-2 flex">
      <Button
        ref={ref}
        variant="ghost"
        size="icon"
        aria-label="Add a Quality"
        title="Add a Quality"
        onClick={onAdd}
      >
        <Plus aria-hidden strokeWidth={1.5} />
      </Button>
    </span>
  );
  return (
    <div className="flex items-center justify-between gap-3">
      <h2 id="qualities" className="font-medium">
        Qualities
      </h2>
      {add}
    </div>
  );
}
