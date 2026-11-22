import { X } from "lucide-react";
import { Button } from "nama-ui/components/button";

/** Takes the row called `label` out of its list, unless `disabled`. */
export function RemoveButton({
  label,
  disabled,
  onRemove,
}: {
  label: string;
  disabled: boolean;
  onRemove: () => void;
}) {
  return (
    <Button
      type="button"
      variant="ghost"
      size="icon-sm"
      aria-label={`Remove ${label}`}
      disabled={disabled}
      onClick={onRemove}
    >
      <X aria-hidden />
    </Button>
  );
}
