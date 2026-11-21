import { Button } from "@/shared/ui/button";

/** The button that takes away the Custom Image of what `name` names, such as "Poster". */
export function RemoveImage({
  name,
  pending,
  onRemove,
}: {
  name: string;
  pending: boolean;
  onRemove: () => void;
}) {
  return (
    <Button
      variant="ghost"
      aria-label={`Remove custom image: ${name}`}
      disabled={pending}
      onClick={onRemove}
    >
      Remove custom image
    </Button>
  );
}
