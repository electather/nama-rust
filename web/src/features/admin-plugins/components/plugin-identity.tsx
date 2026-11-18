import type { PluginSummary } from "@/gen/nama/v1/plugin_pb";
import { PluginLogo } from "@/shared/components/plugin-logo";
import { kindAndVersion } from "@/shared/lib/plugins";

/** A Plugin's logo, id, and where it comes from. */
export function PluginIdentity({ summary }: { summary: PluginSummary }) {
  return (
    <div className="mt-3 flex items-center gap-3">
      <PluginLogo plugin={summary} />
      <p className="text-sm text-muted-foreground">
        <span>{summary.id}</span> · <span>{kindAndVersion(summary)}</span>
      </p>
    </div>
  );
}
