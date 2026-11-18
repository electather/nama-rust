import { useId } from "react";

/** The checkbox that turns resizing on or off, showing the `saved` choice. */
export function ResizeField({ saved }: { saved: boolean }) {
  const hintId = useId();
  return (
    <div className="flex flex-col gap-2">
      <label className="flex items-center gap-3 text-sm font-medium">
        <input
          type="checkbox"
          name="resizeImages"
          defaultChecked={saved}
          aria-describedby={hintId}
          className="size-4 accent-foreground"
        />
        Resize images
      </label>
      <p id={hintId} className="text-sm text-pretty text-muted-foreground">
        Sends each Poster and Backdrop at the size it’s shown, so a phone isn’t sent a picture made
        for a TV. Off, every image is sent at full size and the Server does no image work.
      </p>
    </div>
  );
}
