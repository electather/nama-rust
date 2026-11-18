import { Button } from "@/shared/ui/button";

/** The button that removes the `number`th Quality from the list. */
export function RemoveQuality({ number, onRemove }: { number: number; onRemove: () => void }) {
  return (
    <Button type="button" variant="ghost" onClick={onRemove}>
      Remove<span className="sr-only"> Quality {number}</span>
    </Button>
  );
}
