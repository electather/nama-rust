import { RotateCcw } from "lucide-react";
import { Button } from "nama-ui/components/button";

/**
 * The icon button that forgets the Plugin `name`'s Personal settings. It's pulled into the corner
 * of the heading's row it ends, so it doesn't make that row taller.
 */
export function ForgetButton({
  name,
  disabled,
  onClick,
}: {
  name: string;
  disabled: boolean;
  onClick: () => void;
}) {
  return (
    <span className="-my-2 -me-2 flex">
      <Button
        type="button"
        variant="ghost"
        size="icon-sm"
        aria-label={`Forget your ${name} settings`}
        title="Forget these settings"
        disabled={disabled}
        onClick={onClick}
      >
        <RotateCcw aria-hidden strokeWidth={1.5} />
      </Button>
    </span>
  );
}
