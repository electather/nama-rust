import { FlaskConical } from "lucide-react";

import type { Plugin } from "@/gen/nama/v1/plugin_pb";
import { Button } from "@/shared/ui/button";

/**
 * The icon button that hands `onTest` the settings typed in its form, for a `plugin` with a check.
 * It's pulled into the corner of the heading's row it ends, so it doesn't make that row taller.
 */
export function TestButton({
  plugin,
  pending,
  onTest,
}: {
  plugin: Plugin;
  pending: boolean;
  onTest: (typed: FormData) => void;
}) {
  if (!plugin.checksSettings) {
    return null;
  }
  return (
    <span className="-my-2 -me-2 flex">
      <Button
        type="button"
        variant="ghost"
        size="icon-sm"
        aria-label="Test the settings"
        title="Test the settings"
        aria-busy={pending}
        disabled={pending}
        onClick={(event) => {
          const { form } = event.currentTarget;
          if (form) {
            onTest(new FormData(form));
          }
        }}
      >
        <FlaskConical aria-hidden strokeWidth={1.5} />
      </Button>
    </span>
  );
}
