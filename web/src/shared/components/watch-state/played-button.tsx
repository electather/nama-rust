import { PlayedIcon } from "@/shared/components/watch-state/played-icon";
import { Button } from "@/shared/ui/button";

/** A "Played" button that toggles whether you've played a title. */
export function PlayedButton({
  played,
  onToggle,
}: {
  played: boolean;
  onToggle: (played: boolean) => void;
}) {
  return (
    <Button
      variant="ghost"
      aria-pressed={played}
      onClick={() => {
        onToggle(!played);
      }}
    >
      <PlayedIcon played={played} />
      Played
    </Button>
  );
}
