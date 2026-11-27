import { ContextMenu, ContextMenuTrigger } from "nama-ui/components/context-menu";
import type { ReactNode } from "react";

import { CardCaption } from "@/shared/components/backdrop-card/card-caption";
import { TileArt } from "@/shared/components/tile-art";
import type { OfferedWidths } from "@/shared/hooks/use-src-set";
import { backdropRadius } from "@/shared/lib/backdrop-card-classes";

/**
 * A card's box: its art, 4:3, or square when `tall`, with `caption` and `menu` at its bottom, over
 * `children`, the link that covers it and names it. A right-click opens the context menu `menu`
 * puts in it, the same items as its "…" (`ContextMenuContent`). `onArtError` hears when its art
 * fails to load.
 */
export function BackdropFrame({
  art,
  tall = false,
  caption,
  menu,
  children,
  onArtError,
}: {
  art: {
    url: string | undefined;
    thumbhash: Uint8Array | undefined;
    sizes: string;
    widths: OfferedWidths;
  };
  tall?: boolean;
  caption: ReactNode;
  menu: ReactNode;
  children: ReactNode;
  onArtError?: () => void;
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
        <TileArt
          url={art.url}
          thumbhash={art.thumbhash}
          look={{ sizes: art.sizes, radius: backdropRadius, widths: art.widths }}
          onError={onArtError}
        />
        {children}
        <CardCaption caption={caption} menu={menu} />
      </ContextMenuTrigger>
    </ContextMenu>
  );
}
