import { ServerImg } from "@/shared/components/server-img";
import type { ServerImage } from "@/shared/lib/server-image";

/** A title's whole poster, 2:3 and uncropped, as wide as the slot an icon row leads with. */
export function RowArt({ poster }: { poster: ServerImage }) {
  return (
    <span className="flex w-10 shrink-0 justify-center">
      <span className="relative isolate aspect-2/3 h-14 overflow-hidden rounded-xs bg-muted outline -outline-offset-1 outline-border">
        {/* The row's label names the title, so the image says nothing more. */}
        <ServerImg images={[poster]} sizes="38px" className="size-full object-cover" />
      </span>
    </span>
  );
}
