import type { LinkOptions } from "@tanstack/react-router";
import type { ReactNode } from "react";

import { BackButton } from "@/features/player/components/back-button";
import { TitleLines } from "@/features/player/components/title-lines";

/** Back, the title and the `line` under it, then `actions`, over a shade from the top, with a short `notice` under them. */
export function TopBar({
  title,
  line,
  back,
  actions,
  notice,
  shaded = true,
}: {
  title: string;
  line?: string | undefined;
  back: LinkOptions;
  actions?: ReactNode;
  notice?: ReactNode;
  /** Off over a full scrim, which a shade would darken twice. */
  shaded?: boolean;
}) {
  return (
    <div className={shaded ? "bg-linear-to-b from-scrim from-25% via-scrim/40 pb-16" : "pb-16"}>
      <header className="pointer-events-auto flex items-center gap-3.5 p-5 group-data-controls-hidden:pointer-events-none">
        <BackButton back={back} />
        <TitleLines title={title} line={line} />
        {actions}
      </header>
      {notice}
    </div>
  );
}
