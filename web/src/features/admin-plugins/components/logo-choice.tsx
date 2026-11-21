import { LogoUpload } from "@/features/admin-plugins/components/logo-upload";
import { useChangeLogo } from "@/features/admin-plugins/hooks/use-change-logo";
import type { PluginSummary } from "@/gen/nama/v1/plugin_pb";
import { RemoveImage } from "@/shared/components/change-image/remove-image";

/**
 * Where an admin uploads their own logo for the Plugin, shown in place of its own until removed,
 * and why the Server refused to change it.
 */
export function LogoChoice({ summary }: { summary: PluginSummary }) {
  const { accept, upload, remove, pending, error } = useChangeLogo(summary.id);
  return (
    <section aria-labelledby="logo" className="mt-10">
      <h2 id="logo" className="font-medium">
        Logo
      </h2>
      <p className="mt-1 mb-3 max-w-prose text-sm text-pretty text-muted-foreground">
        {summary.customLogo
          ? "Everyone sees the logo you uploaded, until you remove it."
          : "Upload your own to show in place of the Plugin’s."}
      </p>
      <LogoUpload accept={accept} pending={pending} onFile={upload} />
      {summary.customLogo && (
        <div className="-ms-3 mt-2 flex">
          <RemoveImage name="Logo" pending={pending} onRemove={remove} />
        </div>
      )}
      {error !== undefined && (
        <p role="alert" className="mt-2 text-sm text-pretty text-destructive">
          {error}
        </p>
      )}
    </section>
  );
}
