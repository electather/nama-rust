import { pluginQueriesKey } from "@/features/admin-plugins/data/plugin-queries-key";
import { useChangeImage } from "@/shared/hooks/use-change-image";
import type { ImageRules } from "@/shared/lib/image-rules";

/** A Plugin's logo may also be an SVG, and the Plugin queries send its URL. */
const pluginLogos: ImageRules = {
  accept: "image/jpeg,image/png,image/webp,image/svg+xml",
  refusals: {
    403: "This needs the Manage Settings Permission.",
    413: "That file is too big: an SVG can be up to 64 KB, other images up to 20 MB.",
    415: "That isn’t a JPEG, PNG, WebP or SVG image.",
    422: "That image is over 12 megapixels. Pick a smaller one.",
  },
  queryKey: pluginQueriesKey,
};

/**
 * Uploads the image an admin shows in place of the Plugin `id`'s logo, or removes it, and says why
 * the Server refused, with the types of file a file input should `accept` for it.
 */
export function useChangeLogo(id: string) {
  const path = `/custom-images/plugin/${encodeURIComponent(id)}/logo`;
  const { upload, change, pending, error } = useChangeImage(path, pluginLogos);
  return {
    accept: pluginLogos.accept,
    upload,
    remove: () => {
      void change({ method: "DELETE" });
    },
    pending,
    error,
  };
}
