import type { ImageSettings } from "@/gen/nama/v1/server_pb";
import type { SettingText } from "@/shared/lib/setting-text";

/** The Server pane's settings, as its controls hold them. */
export interface ServerValues {
  url: string;
  resize: boolean;
  /** The most disk the resized copies take, in MiB, as its dropdown is valued. */
  space: string;
}

/**
 * What each setting's row says, in the pane's order, the image store's included. The toast names a
 * refused change by its label.
 */
export const serverLabels: Record<keyof ServerValues | "imageStore", SettingText> = {
  url: {
    label: "Server URL",
    hint: "The address people reach this Server at. Links in messages and Invites start with it. Leave it empty for no links, and no Invites. It’s saved for everyone when you leave the field.",
    words: "domain",
  },
  resize: {
    label: "Resize images",
    hint: "Sends each Poster and Backdrop at the size it’s shown, so a phone isn’t sent a picture made for a TV. Off, every image is sent at full size.",
    words: "thumbnails",
  },
  space: {
    label: "Space for resized images",
    hint: "The most disk the resized copies take. Lowering it removes the oldest; they’re made again when needed.",
    words: "cache storage",
  },
  imageStore: {
    label: "Image store",
    hint: "Where the Server keeps the images it fetched, Custom Images and Plugin logos. Resized copies always stay on local disk.",
    words: "blob s3 storage",
  },
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
