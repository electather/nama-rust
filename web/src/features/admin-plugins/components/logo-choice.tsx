import { pluginQueriesKey } from "@/features/admin-plugins/data/plugin-queries-key";
import type { PluginSummary } from "@/gen/nama/v1/plugin_pb";
import { ChangeImage } from "@/shared/components/change-image/change-image";
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

/** Where an admin uploads their own logo for the Plugin, shown in place of its own until removed. */
export function LogoChoice({ summary }: { summary: PluginSummary }) {
  const image = {
    name: "Logo",
    custom: summary.customLogo,
    path: `/custom-images/plugin/${encodeURIComponent(summary.id)}/logo`,
  };
  return (
    <section aria-labelledby="logo" className="mt-10">
      <h2 id="logo" className="font-medium">
        Logo
      </h2>
      <p className="mt-1 max-w-prose text-sm text-pretty text-muted-foreground">
        {summary.customLogo
          ? "Everyone sees the logo you uploaded, until you remove it."
          : "Upload your own to show in place of the Plugin’s. JPEG, PNG or WebP up to 20 MB, or SVG up to 64 KB."}
      </p>
      <ChangeImage image={image} rules={pluginLogos} />
    </section>
  );
}
