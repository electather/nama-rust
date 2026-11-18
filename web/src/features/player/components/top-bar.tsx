import type { LinkOptions } from "@tanstack/react-router";
import type { ReactNode } from "react";

import { BackButton } from "@/features/player/components/back-button";

/** The title bar, with a short `notice` under it. */
export function TopBar({
  title,
  back,
  playbackInfo,
  notice,
}: {
  title: string;
  back: LinkOptions;
  playbackInfo?: ReactNode;
  notice?: ReactNode;
}) {
  return (
    <div className="bg-linear-to-b from-scrim pb-10">
      <header className="pointer-events-auto flex items-center gap-2 px-2 pt-2 group-data-controls-hidden:pointer-events-none sm:px-4 sm:pt-4">
        <BackButton back={back} />
        <h1 className="me-auto truncate font-medium">{title}</h1>
        {playbackInfo}
      </header>
      {notice}
    </div>
  );
}
