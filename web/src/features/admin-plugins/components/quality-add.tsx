import { useEffect, useRef } from "react";

import { QualityFields } from "@/features/admin-plugins/components/quality-fields";
import type { Choice } from "@/gen/nama/v1/plugin_pb";
import { onSubmitted } from "@/shared/lib/forms";
import { Button } from "@/shared/ui/button";

const BLANK = { profileId: "", name: "", description: "" };

/**
 * The row a new Quality is typed into, at the end of the list. Unlike a saved one it's sent only
 * when asked, as the Server takes no Quality without a profile and a name.
 */
export function QualityAdd({
  profiles,
  pending,
  onAdd,
  onCancel,
}: {
  profiles: Choice[] | undefined;
  pending: boolean;
  onAdd: (typed: FormData) => void;
  onCancel: () => void;
}) {
  const form = useRef<HTMLFormElement>(null);
  useEffect(() => {
    // It opened from the button over the list, so focus comes down to its first field.
    form.current?.querySelector("select")?.focus();
  }, []);
  const end = (
    <div className="flex justify-end gap-2">
      <Button type="button" variant="ghost" onClick={onCancel}>
        Cancel
      </Button>
      <Button type="submit" variant="secondary" disabled={pending}>
        Add
      </Button>
    </div>
  );
  return (
    <li className="py-3.5 not-first:border-t">
      <form ref={form} className="flex flex-col gap-2" onSubmit={onSubmitted(onAdd)}>
        <QualityFields quality={BLANK} profiles={profiles} />
        {end}
      </form>
    </li>
  );
}
