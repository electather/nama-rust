import type { Plugin } from "@/gen/nama/v1/plugin_pb";
import { Button } from "@/shared/ui/button";

/** The button that hands `onTest` the settings typed in its form, for a `plugin` with a check. */
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
    <Button
      type="button"
      variant="ghost"
      disabled={pending}
      onClick={(event) => {
        const { form } = event.currentTarget;
        if (form) {
          onTest(new FormData(form));
        }
      }}
    >
      {pending ? "Testing…" : "Test"}
    </Button>
  );
}
