import { X } from "lucide-react";
import { Button } from "nama-ui/components/button";
import { type ComponentType, Suspense } from "react";

import { Crumbs } from "@/features/settings/components/crumbs";
import { JobCrumbs } from "@/features/settings/components/job-crumbs";
import { PluginCrumbs } from "@/features/settings/components/plugin-crumbs";
import { ResetHomeLayout } from "@/features/settings/components/reset-home-layout";
import { type Section, sectionTitles } from "@/features/settings/lib/sections";

// The sections that open one of their things in place of their list, whose crumbs then name it.
const openedCrumbs: Partial<Record<Section, ComponentType>> = {
  plugins: PluginCrumbs,
  jobs: JobCrumbs,
};

/**
 * The top of the pane: where in Settings it is, what acts on the whole pane, such as going back to
 * the default Home, and the button that closes it.
 */
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
  const Opened = openedCrumbs[section];
  const crumbs = Opened ? (
    <Suspense fallback={plain}>
      <Opened />
    </Suspense>
  ) : (
    plain
  );
  return (
    // First on a phone, over the strip of sections that comes before it in the page.
    <header className="flex h-11 shrink-0 items-center gap-1 ps-4 pe-1.5 max-md:order-first md:ps-5">
      {/* The dialog is named by this alone, whichever section is open. */}
      <h2 id={titleId} className="sr-only">
        Settings
      </h2>
      <nav aria-label="Breadcrumbs" className="me-auto min-w-0 pe-1">
        {crumbs}
      </nav>
      {section === "home" && <ResetHomeLayout />}
      <Button variant="ghost" size="icon-sm" aria-label="Close Settings" onClick={onClose}>
        <X aria-hidden strokeWidth={1.5} />
      </Button>
    </header>
  );
}
