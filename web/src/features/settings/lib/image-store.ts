import type { ConnectError } from "@connectrpc/connect";

import { ExtensionPoint, type PluginSummary } from "@/gen/nama/v1/plugin_pb";
import { RefusalReason } from "@/gen/nama/v1/refusal_pb";
import type { ImageStoreSwitch } from "@/gen/nama/v1/server_pb";
import { pluginName } from "@/lib/plugins";
import { endSentence, refusal, refusalOf } from "@/shared/lib/forms";

/** Whether `switching` is still copying, rather than stopped with a failure. */
export function isRunning(switching: ImageStoreSwitch | undefined) {
  return switching?.failure === "";
}

/** Whether `plugin` may keep the images: an enabled Blob store Plugin. */
export function isEnabledBlobStore(plugin: PluginSummary) {
  return plugin.enabled && plugin.extensionPoints.includes(ExtensionPoint.BLOB_STORE);
}

/** What admins know the store `pluginId` by: "Local disk" for none, else its Plugin's name. */
export function storeName(pluginId: string, plugins: PluginSummary[]) {
  if (pluginId === "") {
    return "Local disk";
  }
  return pluginName(plugins.find((plugin) => plugin.id === pluginId) ?? { id: pluginId, name: "" });
}

/** Why the Server refused to switch the image store, by the Refusal's reason. */
export function switchRefusal(error: ConnectError, nameOf: (pluginId: string) => string) {
  const { params } = refusalOf(error);
  const named = nameOf(params["pluginId"] ?? "");
  return refusal(
    error,
    "switch the image store",
    {},
    {
      [RefusalReason.IMAGE_STORE_FAILED_CHECK]: {
        form: `${named} failed its check, so the images stay where they are: ${endSentence(params["reason"] ?? "it couldn’t keep an image")}`,
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
