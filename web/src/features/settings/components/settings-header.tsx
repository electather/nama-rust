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
import { SettingsCrumb } from "@/features/settings/components/settings-crumb";
import { UserCrumbs } from "@/features/settings/components/user-crumbs";
import { CrumbsRoot } from "@/features/settings/hooks/crumbs-root";
import { sectionTitles } from "@/features/settings/lib/sections";
import { ErrorBoundary } from "@/shared/components/error-boundary";
import type { Section } from "@/shared/lib/settings-address";

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
 * `served`, as for a Section the Server in use doesn't serve, nothing acts on the pane and the
 * crumbs name only the Section: both may call what the Server lacks. A crumb whose read fails also
 * falls back to the Section's name, since this sits outside the pane's own error boundary. On a
 * phone, over the list of sections (`phone` "list") it says Settings alone, and over a section
 * ("section") its trail starts with the Settings crumb back to that list.
 */
export function SettingsHeader({
  titleId,
  section,
  served,
  phone,
  onClose,
}: {
  titleId: string;
  section: Section;
  served: boolean;
  phone?: "list" | "section" | undefined;
  onClose: () => void;
}) {
  const plain = <Crumbs current={phone === "list" ? "Settings" : sectionTitles[section]} />;
  const onSection = served && phone !== "list";
  const Opened = onSection ? openedCrumbs[section] : undefined;
  const Action = onSection ? sectionActions[section] : undefined;
  const crumbs = Opened ? (
    <ErrorBoundary key={section} failed={plain}>
      <Suspense fallback={plain}>
        <Opened />
      </Suspense>
    </ErrorBoundary>
  ) : (
    plain
  );
  const root = phone === "section" ? <SettingsCrumb /> : undefined;
  const trail = <CrumbsRoot value={root}>{crumbs}</CrumbsRoot>;
  return (
    <header className="flex h-11 shrink-0 items-center gap-1 ps-4 pe-1.5 md:ps-5">
      {/* The dialog is named by this alone, whichever section is open. */}
      <h2 id={titleId} className="sr-only">
        Settings
      </h2>
      <nav aria-label="Breadcrumbs" className="me-auto min-w-0 pe-1">
        {trail}
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
