import { TileArt } from "@/shared/components/tile-art";

const radius = "rounded-xl";

/** A Box Set's Poster at the top of its page, as a shelf's poster is drawn. */
export function BoxSetArt({ url }: { url: string }) {
  return (
    <div className={`relative isolate aspect-2/3 w-32 overflow-hidden bg-muted sm:w-40 ${radius}`}>
      <TileArt
        url={url}
        thumbhash={undefined}
        look={{ sizes: "160px", radius, widths: "posterWidths" }}
      />
    </div>
  );
}
