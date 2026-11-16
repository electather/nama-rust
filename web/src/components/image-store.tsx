import type { ConnectError } from "@connectrpc/connect";
import {
  createConnectQueryKey,
  useMutation,
  useQuery,
  useSuspenseQuery,
} from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";

import { FormEnd, FormSelect } from "@/components/form-field";
import { Progress } from "@/components/ui/progress";
import { ExtensionPoint, PluginService, type PluginSummary } from "@/gen/nama/v1/plugin_pb";
import { RefusalReason } from "@/gen/nama/v1/refusal_pb";
import { type ImageStoreSwitch, ServerService } from "@/gen/nama/v1/server_pb";
import { formText, onSubmitted, refusal, refusalOf } from "@/lib/forms";
import { pluginName } from "@/lib/plugins";

const imageSettingsKey = createConnectQueryKey({
  schema: ServerService.method.getImageSettings,
  cardinality: "finite",
});

/** How often to look again while the images are copied to another store. */
const SWITCHING_POLL_MS = 1000;

/** Whether `switching` is still copying, rather than stopped with a failure. */
function isRunning(switching: ImageStoreSwitch | undefined) {
  return switching !== undefined && switching.failure === "";
}

/**
 * Where the Server keeps its stored images, which a User who manages settings switches to local
 * disk or an enabled Blob store Plugin, following the copy as it runs.
 */
export function ImageStore() {
  const { settings, switching } = useSuspenseQuery(
    ServerService.method.getImageSettings,
    {},
    {
      refetchInterval: (query) =>
        isRunning(query.state.data?.switching) ? SWITCHING_POLL_MS : false,
    },
  ).data;
  const stores = useBlobStores();
  const current = settings?.storePluginId ?? "";
  const nameOf = (pluginId: string) => storeName(pluginId, stores);

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
        <p className="mt-3 text-sm">
          In use: <span className="font-medium">{nameOf(current)}</span>
        </p>
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

/** The enabled Blob store Plugins, or none while the list loads. */
function useBlobStores() {
  const { data } = useQuery(PluginService.method.listPlugins, {});
  return (data?.summaries ?? []).filter(
    (plugin) => plugin.enabled && plugin.extensionPoints.includes(ExtensionPoint.BLOB_STORE),
  );
}

/** What admins know the store `pluginId` by: "Local disk" for none, else its Plugin's name. */
function storeName(pluginId: string, stores: PluginSummary[]) {
  if (pluginId === "") return "Local disk";
  return pluginName(stores.find((plugin) => plugin.id === pluginId) ?? { id: pluginId, name: "" });
}

/** How far the copy to another store has got, or why it stopped. Nothing without one. */
function SwitchProgress({
  switching,
  nameOf,
  current,
}: {
  switching: ImageStoreSwitch | undefined;
  nameOf: (pluginId: string) => string;
  current: string;
}) {
  if (switching === undefined) return null;
  const to = nameOf(switching.toPluginId);
  if (isRunning(switching)) return <Copying switching={switching} to={to} />;
  return (
    <p role="alert" className="text-sm text-pretty text-destructive">
      Moving the images to {to} stopped, so they stay on {current}: {switching.failure}.
    </p>
  );
}

/** The running copy to the store shown as `to`: a bar, and how many images it has copied. */
function Copying({
  switching: { copied, total },
  to,
}: {
  switching: ImageStoreSwitch;
  to: string;
}) {
  // The Server lists the images before it copies any, and has none to count until then.
  const listing = total === 0;
  return (
    <div className="flex flex-col gap-1.5">
      <Progress value={listing ? 0 : (copied / total) * 100} aria-label="Images copied" />
      <p className="text-sm text-muted-foreground">
        {listing
          ? `Listing the images to copy to ${to}…`
          : `Copied ${copied} of ${total} images to ${to}…`}
      </p>
    </div>
  );
}

/** The picker of stores, `current` chosen, and the button that switches to the one picked. */
function SwitchForm({
  current,
  stores,
  running,
  nameOf,
}: {
  current: string;
  stores: PluginSummary[];
  running: boolean;
  nameOf: (pluginId: string) => string;
}) {
  const queryClient = useQueryClient();
  const switchStore = useMutation(ServerService.method.switchImageStore, {
    onSuccess: () => queryClient.invalidateQueries({ queryKey: imageSettingsKey }),
  });
  const failed = switchStore.error === null ? undefined : switchRefusal(switchStore.error, nameOf);

  return (
    <form
      className="flex flex-col gap-5"
      onSubmit={onSubmitted((typed) => {
        switchStore.mutate({ pluginId: formText(typed, "pluginId") });
      })}
    >
      <FormSelect
        label="Keep images on"
        name="pluginId"
        defaultValue={current}
        // Local disk is the empty value, which a required select would refuse.
        required={false}
        hint="An enabled Blob store Plugin, such as S3, can keep them in place of local disk. Every image is copied across before the switch, and the old store keeps its copies."
      >
        <option value="">Local disk</option>
        {stores.map((plugin) => (
          <option key={plugin.id} value={plugin.id}>
            {pluginName(plugin)}
          </option>
        ))}
      </FormSelect>
      <FormEnd
        action={switchStore.isPending ? "Checking the store…" : "Switch image store"}
        variant="ghost"
        pending={switchStore.isPending || running}
        done={undefined}
        refusal={failed}
      />
    </form>
  );
}

/** Why the Server refused to switch the image store, by the Refusal's reason. */
function switchRefusal(error: ConnectError, nameOf: (pluginId: string) => string) {
  const { params } = refusalOf(error);
  const named = nameOf(params["pluginId"] ?? "");
  return refusal(
    error,
    "switch the image store",
    {},
    {
      [RefusalReason.IMAGE_STORE_FAILED_CHECK]: {
        form: `${named} failed its check, so the images stay where they are: ${params["reason"] ?? "it couldn’t keep an image"}.`,
      },
      [RefusalReason.IMAGE_STORE_IN_USE]: { form: `${named} already keeps the images.` },
      [RefusalReason.IMAGE_STORE_SWITCHING]: {
        form: "The images are already moving to another store. Wait for that to finish.",
      },
      [RefusalReason.NOT_A_BLOB_STORE]: {
        form: `${named} isn’t an enabled Blob store Plugin any more.`,
      },
    },
  ).form;
}
