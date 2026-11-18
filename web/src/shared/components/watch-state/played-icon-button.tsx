import { PlayedIcon } from "@/shared/components/watch-state/played-icon";
import { Button } from "@/shared/ui/button";

/** `PlayedButton` as an icon alone, named by `label`. */
export function PlayedIconButton({
  played,
  label,
  onToggle,
}: {
  played: boolean;
  label: string;
  onToggle: (played: boolean) => void;
}) {
  return (
    <Button
      variant="ghost"
      size="icon"
      aria-label={label}
      aria-pressed={played}
      onClick={() => {
        onToggle(!played);
      }}
    >
      <PlayedIcon played={played} />
    </Button>
  );
}
