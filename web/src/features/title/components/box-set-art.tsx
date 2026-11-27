import { ServerImg } from "@/shared/components/server-img";
import type { ServerImage } from "@/shared/lib/server-image";

const radius = "rounded-xl";

/** A Box Set's Poster at the top of its page, as a shelf's poster is drawn. */
export function BoxSetArt({ poster }: { poster: ServerImage }) {
  return (
    <div className={`relative isolate aspect-2/3 w-32 overflow-hidden bg-muted sm:w-40 ${radius}`}>
      <ServerImg
        images={[poster]}
        sizes="160px"
        className="absolute inset-0 size-full object-cover"
      />
      <span aria-hidden className={`absolute inset-0 inset-ring inset-ring-border ${radius}`} />
    </div>
  );
}
