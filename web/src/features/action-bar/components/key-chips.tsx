import { Kbd } from "@/features/action-bar/components/kbd";

/** A shortcut's keys, in the order they're pressed. */
export function KeyChips({ keys }: { keys: string[] }) {
  return (
    <span className="inline-flex shrink-0 gap-1">
      {keys.map((key) => (
        <Kbd key={key}>{key}</Kbd>
      ))}
    </span>
  );
}
