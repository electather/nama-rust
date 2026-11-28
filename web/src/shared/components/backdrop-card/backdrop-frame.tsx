import { ContextMenu, ContextMenuTrigger } from "nama-ui/components/context-menu";
import type { ReactNode } from "react";

import { CardCaption } from "@/shared/components/backdrop-card/card-caption";
import { ServerImg } from "@/shared/components/server-img";
import { followPointer } from "@/shared/lib/follow-pointer";
import type { ServerImage } from "@/shared/lib/server-image";
import { backdropTile, tileMotion, tileRing } from "@/shared/lib/tile-shapes";

/**
 * A card's box: its art, the first of `art.images` that loads, 4:3, or square when `tall`, with
 * `caption` and `menu` at its bottom, over `children`, the link that covers it and names it. A
 * right-click opens the context menu `menu` puts in it, the same items as its "…"
 * (`ContextMenuContent`).
 */
export function BackdropFrame({
  art,
  tall = false,
  caption,
  menu,
  children,
}: {
  art: { images: (ServerImage | undefined)[]; sizes: string };
  tall?: boolean;
  caption: ReactNode;
  menu: ReactNode;
  children: ReactNode;
}) {
  return (
    <ContextMenu>
      <ContextMenuTrigger
        render={
          <div
            onPointerMove={followPointer}
            className={`relative isolate overflow-hidden bg-muted ${tileMotion} ${tall ? backdropTile.tallAspect : backdropTile.aspect} ${backdropTile.radius}`}
          />
        }
      >
        <ServerImg {...art} fades className="absolute inset-0 size-full object-cover" />
        <span aria-hidden className={`${tileRing} ${backdropTile.radius}`} />
        {children}
        <CardCaption caption={caption} menu={menu} />
      </ContextMenuTrigger>
    </ContextMenu>
  );
}
