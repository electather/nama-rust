import type { ImageSettings } from "@/gen/nama/v1/server_pb";

/** The Server pane's settings, as its controls hold them. */
export interface ServerValues {
  url: string;
  resize: boolean;
  /** The most disk the resized copies take, in MiB, as its dropdown is valued. */
  space: string;
}

/** Each setting's name, as its row says it, and its pane's toast when a change to it is refused. */
export const serverLabels: Record<keyof ServerValues, string> = {
  url: "Server URL",
  resize: "Resize images",
  space: "Space for resized images",
};

// What the Server keeps when the setting is unset.
const DEFAULT_MIB = 1024;
const MIB_PER_GB = 1024;
const offeredGb = [1, 2, 5, 10, 20];

/** The pane's settings as the Server has them saved. */
export function savedServer(url: string, images: ImageSettings | undefined): ServerValues {
  // Resizing is on until the Server says otherwise, as it is by default.
  const { resizeImages = true, resizedCacheMib = DEFAULT_MIB } = images ?? {};
  return { url, resize: resizeImages, space: String(resizedCacheMib) };
}

/** The sizes to pick from, with the `saved` one if it's none of them, so opening the pane never changes it. */
export function spaceOptions(saved: string) {
  const offered = offeredGb.map((gb) => ({ value: String(gb * MIB_PER_GB), label: `${gb} GB` }));
  const listed = offered.some(({ value }) => value === saved);
  return listed ? offered : [{ value: saved, label: `${saved} MB` }, ...offered];
}

/** What's wrong with the Server URL `typed`, before the Server is asked. Empty is fine: it means none. */
export function urlProblem(typed: string) {
  const url = typed.trim();
  const full = url === "" || (/^https?:\/\//iu.test(url) && URL.canParse(url));
  return full ? undefined : "Enter a full address, like https://media.example.com.";
}
