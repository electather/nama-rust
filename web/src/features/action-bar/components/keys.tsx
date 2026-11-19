import { KeyCap } from "@/features/action-bar/components/key-cap";

/** The keys of one shortcut, side by side, in the order they're pressed. Nothing for no keys. */
export function Keys({ keys }: { keys: readonly string[] | undefined }) {
  if (keys === undefined) {
    return null;
  }
  return (
    <span className="inline-flex shrink-0 gap-1">
      {keys.map((name) => (
        <KeyCap key={name} name={name} />
      ))}
    </span>
  );
}
