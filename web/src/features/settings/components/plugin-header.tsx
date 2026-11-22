import { Link } from "@tanstack/react-router";
import { ArrowLeft, Ban } from "lucide-react";
import { useEffect, useRef } from "react";

import { useChangeLogo } from "@/features/admin-plugins/hooks/use-change-logo";
import { LogoEdit } from "@/features/settings/components/logo-edit";
import { PluginActions } from "@/features/settings/components/plugin-actions";
import { PluginMore } from "@/features/settings/components/plugin-more";
import { PluginLogo } from "@/shared/components/plugin-logo";
import { StateLabel } from "@/shared/components/state-label";
import { kindAndVersion, type PluginDetails, pluginName } from "@/shared/lib/plugins";

/**
 * The top of a Plugin's view, over a hairline: the link back to the list, then its logo with the
 * pencil that changes it, its name over its id and where it comes from, and at the end its "…" and,
 * while it's disabled, Enable. One the Server refused says so there instead, and keeps its logo. Where there's room it stays at the top of the pane while
 * the rest scrolls under it.
 */
export function PluginHeader({ titleId, plugin }: { titleId: string; plugin: PluginDetails }) {
  const { summary } = plugin;
  const logo = useChangeLogo(summary.id);
  const title = useRef<HTMLHeadingElement>(null);
  useEffect(() => {
    // The list this replaced held focus, so it moves to what took its place.
    title.current?.focus();
  }, []);
  const back = (
    <Link
      to="."
      search={(rest) => ({ ...rest, plugin: undefined })}
      // The list's search is a subset of the open Plugin's, so inclusive matching marks this current.
      activeOptions={{ exact: true }}
      className="-ms-1 mb-3.5 flex min-h-11 w-fit items-center gap-1.5 rounded-sm px-1 text-sm font-medium outline-none hover:underline focus-visible:ring-3 focus-visible:ring-ring/50 md:min-h-8"
    >
      <ArrowLeft aria-hidden strokeWidth={1.5} className="size-4 rtl:rotate-180" />
      Plugins
    </Link>
  );
  const name = (
    <div className="flex min-w-0 flex-1 basis-50 flex-col gap-0.5">
      <h3
        ref={title}
        id={titleId}
        tabIndex={-1}
        className="truncate text-2xl font-semibold tracking-tight outline-none"
      >
        {pluginName(summary)}
      </h3>
      <p className="text-sm text-muted-foreground">
        {summary.id} · {kindAndVersion(summary)}
      </p>
    </div>
  );
  const refused = (
    <div className="ms-auto flex items-center gap-2">
      <StateLabel label="Refused" Icon={Ban} />
      <PluginMore plugin={plugin} />
    </div>
  );
  const { start, end } =
    summary.refusedReason === ""
      ? {
          start: <LogoEdit summary={summary} logo={logo} />,
          end: <PluginActions plugin={plugin} />,
        }
      : { start: <PluginLogo large plugin={summary} />, end: refused };
  return (
    <header data-sticks className="top-0 z-10 bg-background px-4 pt-4 md:sticky md:px-7.5 md:pt-5">
      {back}
      <div className="flex flex-wrap items-center gap-x-4 gap-y-2.5 border-b pb-6">
        {start}
        {name}
        {end}
        <p role="alert" className="w-full text-sm text-pretty text-destructive empty:hidden">
          {logo.error}
        </p>
      </div>
    </header>
  );
}
