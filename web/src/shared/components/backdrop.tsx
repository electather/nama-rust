import { ServerImg } from "@/shared/components/server-img";
import type { ServerImage } from "@/shared/lib/server-image";

/**
 * The `backdrop`, under its thumbhash until it loads, on the scrim text over it sits on, filling
 * its `relative isolate` box behind what comes after it. `sizes` is how wide it shows, for its
 * `srcset`. Nothing without one.
 */
export function Backdrop({
  backdrop,
  sizes,
}: {
  backdrop: ServerImage | undefined;
  sizes: string;
}) {
  if (backdrop === undefined) {
    return null;
  }
  return (
    <>
      <ServerImg
        images={[backdrop]}
        sizes={sizes}
        largest
        className="absolute inset-0 -z-10 size-full object-cover"
      />
      <div aria-hidden className="absolute inset-0 -z-10 title-scrim" />
    </>
  );
}
