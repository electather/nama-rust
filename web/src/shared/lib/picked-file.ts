import type { ChangeEvent } from "react";

/**
 * Hands the file just picked in a file input to `onPicked`, and clears the input, so picking the
 * same file again after a refusal still hands it over.
 */
export function handPicked(event: ChangeEvent<HTMLInputElement>, onPicked: (file: File) => void) {
  const file = event.target.files?.[0];
  event.target.value = "";
  if (file) {
    onPicked(file);
  }
}
