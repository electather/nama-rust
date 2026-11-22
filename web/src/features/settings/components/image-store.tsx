import { StoreInUse } from "@/features/settings/components/store-in-use";
import { SwitchForm } from "@/features/settings/components/switch-form";
import { SwitchProgress } from "@/features/settings/components/switch-progress";
import { useImageStore } from "@/features/settings/data/use-image-store";
import { usePlugins } from "@/features/settings/data/use-plugins";
import { isEnabledBlobStore, isRunning, storeName } from "@/features/settings/lib/image-store";
import { serverLabels } from "@/features/settings/lib/server-values";

/**
 * Where the Server keeps its stored images, which a User who manages settings switches to local
 * disk or an enabled Blob store Plugin, following the copy as it runs.
 */
export function ImageStore() {
  const { settings, switching } = useImageStore();
  const plugins = usePlugins();
  const stores = plugins.filter((plugin) => isEnabledBlobStore(plugin));
  const current = settings?.storePluginId ?? "";
  // A disabled Plugin may still hold the images, or have been a failed switch's target.
  const nameOf = (pluginId: string) => storeName(pluginId, plugins);

  return (
    <section
      aria-labelledby="image-store"
      data-setting="imageStore"
      className="flex flex-col gap-3.5"
    >
      <div>
        <h4 id="image-store" className="font-medium">
          {serverLabels.imageStore.label}
        </h4>
        <p className="text-xs text-pretty text-muted-foreground">{serverLabels.imageStore.hint}</p>
        <StoreInUse name={nameOf(current)} />
      </div>
      <SwitchProgress switching={switching} nameOf={nameOf} current={nameOf(current)} />
      <SwitchForm
        key={current}
        current={current}
        stores={stores}
        running={isRunning(switching)}
        nameOf={nameOf}
      />
    </section>
  );
}
