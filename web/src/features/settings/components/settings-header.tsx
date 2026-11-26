import { X } from "lucide-react";
import { Button } from "nama-ui/components/button";
import { type ComponentType, Suspense } from "react";

import { Crumbs } from "@/features/settings/components/crumbs";
import { JobCrumbs } from "@/features/settings/components/job-crumbs";
import { JobRunNow } from "@/features/settings/components/job-run-now";
import { LibraryCrumbs } from "@/features/settings/components/library-crumbs";
import { NewLibraryLink } from "@/features/settings/components/new-library-link";
import { PluginCrumbs } from "@/features/settings/components/plugin-crumbs";
import { ResetHomeLayout } from "@/features/settings/components/reset-home-layout";
import { UserCrumbs } from "@/features/settings/components/user-crumbs";
import { type Section, sectionTitles } from "@/features/settings/lib/sections";

// The sections that open one of their things in place of their list, whose crumbs then name it.
const openedCrumbs: Partial<Record<Section, ComponentType>> = {
  plugins: PluginCrumbs,
  libraries: LibraryCrumbs,
  jobs: JobCrumbs,
  users: UserCrumbs,
};

// What acts on a section's whole pane, beside the close button.
const sectionActions: Partial<Record<Section, ComponentType>> = {
  home: ResetHomeLayout,
  libraries: NewLibraryLink,
  jobs: JobRunNow,
};

/**
 * The top of the pane: where in Settings it is, what acts on the whole pane, such as going back to
 * the default Home, adding a Library or running the open Job, and the button that closes it. Unless
 * `served`, as for a Section the Server in use doesn't serve, nothing acts on the pane: that may
 * call what the Server lacks.
 */
export function SettingsHeader({
  titleId,
  section,
  served,
  onClose,
}: {
  titleId: string;
  section: Section;
  served: boolean;
  onClose: () => void;
}) {
  const plain = <Crumbs current={sectionTitles[section]} />;
  const Opened = openedCrumbs[section];
  const Action = served ? sectionActions[section] : undefined;
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
      {Action && (
        <Suspense fallback={null}>
          <Action />
        </Suspense>
      )}
      <Button variant="ghost" size="icon-sm" aria-label="Close Settings" onClick={onClose}>
        <X aria-hidden strokeWidth={1.5} />
      </Button>
    </header>
  );
}
