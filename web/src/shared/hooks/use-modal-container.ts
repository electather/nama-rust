import { useState } from "react";

/**
 * Where a dropdown's list goes when its field may be in a modal dialog, such as Settings: inside
 * that dialog, as only what's inside one shows over it. Gives the ref for an element by the field,
 * then the dialog it's in, if it's in one.
 */
export function useModalContainer() {
  const [inside, setInside] = useState<HTMLElement | null>(null);
  return [setInside, inside?.closest("dialog") ?? undefined] as const;
}
