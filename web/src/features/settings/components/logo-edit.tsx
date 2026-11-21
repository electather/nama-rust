import { useRef } from "react";

import type { useChangeLogo } from "@/features/admin-plugins/hooks/use-change-logo";
import { LogoMenu } from "@/features/settings/components/logo-menu";
import type { PluginSummary } from "@/gen/nama/v1/plugin_pb";
import { PluginLogo } from "@/shared/components/plugin-logo";
import { handPicked } from "@/shared/lib/picked-file";

/**
 * A Plugin's logo at the top of its view, with the pencil at its corner that changes it through
 * `logo`: an image picked in the file dialog is uploaded at once, and one already standing in for
 * the Plugin's own can be removed.
 */
export function LogoEdit({
  summary,
  logo,
}: {
  summary: PluginSummary;
  logo: ReturnType<typeof useChangeLogo>;
}) {
  // Outside the menu, which is gone by the time the file dialog answers.
  const file = useRef<HTMLInputElement>(null);
  return (
    <div className="group/logo relative flex shrink-0">
      <PluginLogo large plugin={summary} />
      <LogoMenu
        pending={logo.pending}
        onUpload={() => file.current?.click()}
        onRemove={summary.customLogo ? logo.remove : undefined}
      />
      <input
        ref={file}
        type="file"
        hidden
        accept={logo.accept}
        onChange={(event) => {
          handPicked(event, logo.upload);
        }}
      />
    </div>
  );
}
