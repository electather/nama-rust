import type { RefObject } from "react";

/**
 * The pop-up that plays a Trailer, named `name`, in its host's own player at `player`, closed by
 * Escape or a click outside it. The player loads only while it's open, so the host hears nothing
 * until a User asks.
 */
export function TrailerDialog({
  ref,
  name,
  player,
  onClose,
}: {
  ref: RefObject<HTMLDialogElement | null>;
  name: string;
  player: string | undefined;
  onClose: () => void;
}) {
  // ponytail: the native modal <dialog> traps focus, closes on Escape and returns focus itself.
  return (
    // The player fills the dialog, so a click on the dialog itself is one on its backdrop. Escape
    // is the keyboard's way out. `closedby="any"` would do this, but Safari lacks it.
    // oxlint-disable-next-line jsx-a11y/click-events-have-key-events, jsx-a11y/no-noninteractive-element-interactions -- a backdrop click, see above
    <dialog
      ref={ref}
      aria-label={name}
      onClose={onClose}
      onClick={(event) => {
        if (event.target === event.currentTarget) {
          ref.current?.close();
        }
      }}
      className="dark m-auto w-full max-w-5xl overflow-hidden rounded-2xl bg-background shadow-xl backdrop:bg-scrim"
    >
      {player !== undefined && (
        <iframe
          src={player}
          title={name}
          allow="autoplay; encrypted-media; fullscreen; picture-in-picture"
          allowFullScreen
          // What the host's player needs to play, go full screen and open the video on its site.
          // oxlint-disable-next-line react/iframe-missing-sandbox -- the player is on its host's origin, never ours, so allow-same-origin gives it only its own
          sandbox="allow-scripts allow-same-origin allow-presentation allow-popups"
          className="block aspect-video w-full"
        />
      )}
    </dialog>
  );
}
