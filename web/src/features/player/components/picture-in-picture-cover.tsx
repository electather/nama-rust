/**
 * What the page shows where the video was while it plays in a window of its own: its backdrop,
 * dimmed, saying so. Shown by CSS alone, since `:picture-in-picture` says when.
 */
export function PictureInPictureCover({ backdropUrl }: { backdropUrl: string | undefined }) {
  return (
    <div className="pointer-events-none absolute inset-0 hidden items-center justify-center bg-background group-has-picture-in-picture:flex">
      <img
        src={backdropUrl}
        alt=""
        className="absolute inset-0 size-full object-cover opacity-25"
      />
      <p className="relative text-sm text-muted-foreground">Playing in picture in picture</p>
    </div>
  );
}
