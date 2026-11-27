import { ContextMenu, ContextMenuTrigger } from "nama-ui/components/context-menu";
import type { ReactNode } from "react";

import { CardCaption } from "@/shared/components/backdrop-card/card-caption";
import { ServerImg } from "@/shared/components/server-img";
import { backdropRadius } from "@/shared/lib/backdrop-card-classes";
import type { ServerImage } from "@/shared/lib/server-image";

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
            className={`relative isolate overflow-hidden bg-muted ${tall ? "aspect-square" : "aspect-4/3"} ${backdropRadius}`}
          />
        }
      >
        <ServerImg {...art} className="absolute inset-0 size-full object-cover" />
        <span
          aria-hidden
          className={`absolute inset-0 inset-ring inset-ring-border ${backdropRadius}`}
        />
        {children}
        <CardCaption caption={caption} menu={menu} />
      </ContextMenuTrigger>
    </ContextMenu>
  );
}
