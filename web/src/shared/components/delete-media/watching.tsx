/** How many people deleting it stops watching, if anyone. */
export function Watching({ playing }: { playing: number }) {
  if (playing === 0) {
    return null;
  }
  return (
    <p className="font-medium">
      {playing === 1 ? "1 person is" : `${playing} people are`} watching it now. Deleting it stops
      their playback.
    </p>
  );
}
