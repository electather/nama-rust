/**
 * The backdrop over the whole player until the video has a frame, rather than black, fading out
 * once it has. A `poster` would do it, but fits the screen as the video does, with bars, where this
 * fills it as "Getting this ready" does before it.
 */
export function FirstFrameCover({
  backdropUrl,
  framed,
}: {
  backdropUrl: string | undefined;
  framed: boolean;
}) {
  return (
    <img
      src={backdropUrl}
      alt=""
      data-framed={framed}
      className="pointer-events-none absolute inset-0 size-full object-cover transition-opacity duration-300 ease-out-expo data-[framed=true]:opacity-0 motion-reduce:transition-none"
    />
  );
}
