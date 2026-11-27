import { X } from "lucide-react";
import { Button } from "nama-ui/components/button";
import { Suspense } from "react";

import { Crumbs } from "@/features/settings/components/crumbs";
import { SettingsCrumb } from "@/features/settings/components/settings-crumb";
import { CrumbsRoot } from "@/features/settings/hooks/crumbs-root";
import { definitions } from "@/features/settings/lib/sections";
import { ErrorBoundary } from "@/shared/components/error-boundary";
import type { Section } from "@/shared/lib/settings-address";

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
  const { title, crumbs: Opened, action: Action } = definitions[section];
  const plain = <Crumbs current={phone === "list" ? "Settings" : title} />;
  const onSection = served && phone !== "list";
  const crumbs =
    onSection && Opened ? (
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
      {onSection && Action && (
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
