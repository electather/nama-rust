import { Copying } from "@/features/settings/components/copying";
import { isRunning } from "@/features/settings/lib/image-store";
import type { ImageStoreSwitch } from "@/gen/nama/v1/server_pb";
import { endSentence } from "@/shared/lib/forms";

/** How far the copy to another store has got, or why it stopped. Nothing without one. */
export function SwitchProgress({
  switching,
  nameOf,
  current,
}: {
  switching: ImageStoreSwitch | undefined;
  nameOf: (pluginId: string) => string;
  current: string;
}) {
  if (switching === undefined) {
    return null;
  }
  const to = nameOf(switching.toPluginId);
  if (isRunning(switching)) {
    return <Copying switching={switching} to={to} />;
  }
  return (
    <p role="alert" className="text-sm text-pretty text-destructive">
      Moving the images to {to} stopped, so they stay on {current}: {endSentence(switching.failure)}
    </p>
  );
}
