import { Circle, CircleCheck } from "lucide-react";

/** A ticked circle once played, an empty one before: never colour alone. */
export function PlayedIcon({ played }: { played: boolean }) {
  return played ? (
    <CircleCheck aria-hidden className="size-5 text-primary" />
  ) : (
    <Circle aria-hidden className="size-5" />
  );
}
