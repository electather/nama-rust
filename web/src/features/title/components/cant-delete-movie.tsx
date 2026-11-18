import { useId } from "react";

import { Button } from "@/shared/ui/button";

/** The Delete action of the Movie `title`, disabled, saying the Server can't write to `folder`. */
export function CantDeleteMovie({ title, folder }: { title: string; folder: string }) {
  const reason = useId();
  return (
    <>
      <Button variant="ghost" disabled aria-label={`Delete ${title}`} aria-describedby={reason}>
        Delete
      </Button>
      <p id={reason} className="basis-full text-sm wrap-break-word text-muted-foreground">
        {title} can’t be deleted: the Server can’t write to {folder}. Make that folder writable,
        then scan the Library again.
      </p>
    </>
  );
}
