import { X } from "lucide-react";

import { Button } from "@/shared/ui/button";

/** The top of Settings: its title, and the button that closes it. */
export function SettingsHeader({ titleId, onClose }: { titleId: string; onClose: () => void }) {
  return (
    <header className="flex shrink-0 items-center justify-between gap-2 py-1 ps-4 pe-1 md:border-b md:py-1.5 md:ps-6 md:pe-1.5">
      <h2 id={titleId} className="text-lg/none font-semibold">
        Settings
      </h2>
      <Button variant="ghost" size="icon" aria-label="Close Settings" onClick={onClose}>
        <X aria-hidden strokeWidth={1.5} />
      </Button>
    </header>
  );
}
