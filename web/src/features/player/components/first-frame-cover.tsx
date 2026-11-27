import { ServerImg } from "@/shared/components/server-img";
import type { ServerImage } from "@/shared/lib/server-image";

/**
 * The title's `art` over the whole player until the video has a frame, rather than black, fading
 * out once it has. A `poster` would do it, but fits the screen as the video does, with bars, where
 * this fills it as "Getting this ready" does before it.
 */
export function FirstFrameCover({
  art,
  framed,
}: {
  art: ServerImage | undefined;
  framed: boolean;
}) {
  return (
    // Its thumbhash fades with it, rather than showing in the video's bars.
    <div
      data-framed={framed}
      className="pointer-events-none absolute inset-0 isolate transition-opacity duration-300 ease-out-expo data-[framed=true]:opacity-0 motion-reduce:transition-none"
    >
      <ServerImg images={[art]} sizes="100vw" className="size-full object-cover" />
    </div>
  );
}
