import { Link, type LinkOptions } from "@tanstack/react-router";
import type { ReactNode } from "react";

import { BackdropFrame } from "@/shared/components/backdrop-card/backdrop-frame";
import type { OfferedWidths } from "@/shared/hooks/use-src-set";
import { backdropLink } from "@/shared/lib/backdrop-card-classes";

/**
 * A card of a title's or Episode's art that opens `link`, named `label`, such as a Continue
 * Watching card or an Episode's, with `caption` and `menu`, its "…", at its bottom. It's as wide
 * as its shelf's tile, 4:3, or square when `tall`, for a caption with a description, so the art
 * still shows above it.
 */
export function BackdropCard({
  art,
  link,
  label,
  tall = false,
  caption,
  menu,
}: {
  art: {
    url: string | undefined;
    thumbhash: Uint8Array | undefined;
    sizes: string;
    widths?: OfferedWidths | undefined;
  };
  link: LinkOptions;
  label: string;
  tall?: boolean;
  caption: ReactNode;
  menu: ReactNode;
}) {
  return (
    <BackdropFrame art={art} tall={tall} caption={caption} menu={menu}>
      <Link {...link} aria-label={label} className={backdropLink} />
    </BackdropFrame>
  );
}
