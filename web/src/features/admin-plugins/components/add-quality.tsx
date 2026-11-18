import { Button } from "@/shared/ui/button";

/** The button that adds an empty Quality to the list. */
export function AddQuality({ onAdd }: { onAdd: () => void }) {
  return (
    // A ghost button is pulled back by its own padding, so its words line up with the fields.
    <div className="-ms-2.5">
      <Button type="button" variant="ghost" onClick={onAdd}>
        Add a Quality
      </Button>
    </div>
  );
}
