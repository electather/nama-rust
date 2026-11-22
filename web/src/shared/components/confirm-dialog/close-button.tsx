import { X } from "lucide-react";
import { Button } from "nama-ui/components/button";

/** A dialog's ✕, in its top corner. */
export function CloseButton({ disabled, onClose }: { disabled: boolean; onClose: () => void }) {
  return (
    <span className="absolute inset-e-2.5 top-2.5">
      <Button variant="ghost" size="close" aria-label="Close" disabled={disabled} onClick={onClose}>
        <X aria-hidden />
      </Button>
    </span>
  );
}
