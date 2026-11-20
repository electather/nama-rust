import type { MessageInitShape } from "@bufbuild/protobuf";

import { type HomeRowSchema, KernelHomeRow } from "@/gen/nama/v1/browse_pb";

/** A Home row as a fake Server's `ListHomeRows` lists it. */
export type HomeRowInit = MessageInitShape<typeof HomeRowSchema>;

/** Continue Watching, the Kernel's row. */
export const continueWatchingRow: HomeRowInit = {
  kernelOrPlugin: { case: "kernel", value: KernelHomeRow.CONTINUE_WATCHING },
};

/** The row `key` that the Plugin `pluginId` gives, under `heading`. */
export function pluginRow(pluginId: string, key: string, heading: string): HomeRowInit {
  return { kernelOrPlugin: { case: "plugin", value: { pluginId, key, heading } } };
}
