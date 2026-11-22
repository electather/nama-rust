import { X } from "lucide-react";
import { Suspense } from "react";

import { Crumbs } from "@/features/settings/components/crumbs";
import { PluginCrumbs } from "@/features/settings/components/plugin-crumbs";
import { type Section, sectionTitles } from "@/features/settings/lib/sections";
import { Button } from "@/shared/ui/button";

/** The top of the pane: where in Settings it is, and the button that closes it. */
export function SettingsHeader({
  titleId,
  section,
  onClose,
}: {
  titleId: string;
  section: Section;
  onClose: () => void;
}) {
  const plain = <Crumbs current={sectionTitles[section]} />;
  const crumbs =
    section === "plugins" ? (
      <Suspense fallback={plain}>
        <PluginCrumbs />
      </Suspense>
    ) : (
      plain
    );
  return (
    // First on a phone, over the strip of sections that comes before it in the page.
    <header className="flex h-12 shrink-0 items-center justify-between gap-2 ps-4 pe-1.5 max-md:order-first md:ps-6">
      {/* The dialog is named by this alone, whichever section is open. */}
      <h2 id={titleId} className="sr-only">
        Settings
      </h2>
      <nav aria-label="Breadcrumbs" className="min-w-0">
        {crumbs}
      </nav>
      <Button variant="ghost" size="icon-sm" aria-label="Close Settings" onClick={onClose}>
        <X aria-hidden strokeWidth={1.5} />
      </Button>
    </header>
  );
}
