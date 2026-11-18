import { StoreInUse } from "@/features/settings/components/store-in-use";
import { SwitchForm } from "@/features/settings/components/switch-form";
import { SwitchProgress } from "@/features/settings/components/switch-progress";
import { useImageStore } from "@/features/settings/data/use-image-store";
import { usePlugins } from "@/features/settings/data/use-plugins";
import { isEnabledBlobStore, isRunning, storeName } from "@/features/settings/lib/image-store";

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
    <section aria-labelledby="image-store" className="mt-10 flex flex-col gap-5">
      <div>
        <h3 id="image-store" className="font-medium">
          Image store
        </h3>
        <p className="mt-1 text-sm text-pretty text-muted-foreground">
          Where the Server keeps the images it fetched, Custom Images and Plugin logos. Resized
          copies always stay on local disk.
        </p>
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
