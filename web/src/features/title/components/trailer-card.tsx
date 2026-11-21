import { useRef, useState } from "react";

import { TrailerDialog } from "@/features/title/components/trailer-dialog";
import { TrailerTile } from "@/features/title/components/trailer-tile";
import { trailerPlayer } from "@/features/title/lib/trailer-player";
import type { Trailer } from "@/gen/nama/v1/request_pb";

/**
 * One of a title's Trailers in its row, over the title's `art`, playing in a pop-up in its host's
 * own player. It's still a link to the host, so a new tab works too.
 */
export function TrailerCard({
  trailer,
  art,
}: {
  trailer: Trailer;
  art: { url: string | undefined; thumbhash: Uint8Array | undefined };
}) {
  const dialog = useRef<HTMLDialogElement>(null);
  const [open, setOpen] = useState(false);
  const player = trailerPlayer(trailer.url);
  return (
    <li className="w-tile-backdrop shrink-0 snap-start">
      <TrailerTile
        trailer={trailer}
        art={art}
        onPlay={
          player === undefined
            ? undefined
            : () => {
                setOpen(true);
                dialog.current?.showModal();
              }
        }
      />
      <TrailerDialog
        ref={dialog}
        name={trailer.name}
        // Only while open, so closing it stops the video.
        player={open ? player : undefined}
        onClose={() => {
          setOpen(false);
        }}
      />
    </li>
  );
}
