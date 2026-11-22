import { Button } from "nama-ui/components/button";

import { QualityFields } from "@/features/admin-plugins/components/quality-fields";
import type { Choice, Quality } from "@/gen/nama/v1/plugin_pb";
import { useAutoSubmit } from "@/shared/hooks/use-auto-submit";
import { onSubmitted } from "@/shared/lib/forms";

/**
 * The fields of a Quality open under its row. Each is handed to `onSave` as it's changed, as the
 * rest of Settings saves, so there's nothing to confirm: Done only closes them.
 */
export function QualityEdit({
  quality,
  profiles,
  onSave,
  onDone,
}: {
  quality: Quality;
  profiles: Choice[] | undefined;
  onSave: (typed: FormData) => void;
  onDone: () => void;
}) {
  const auto = useAutoSubmit();
  return (
    <form className="mt-3 flex flex-col gap-2" {...auto} onSubmit={onSubmitted(onSave)}>
      <QualityFields quality={quality} profiles={profiles} />
      <div className="-me-2.5 flex justify-end">
        <Button type="button" variant="ghost" onClick={onDone}>
          Done
        </Button>
      </div>
    </form>
  );
}
