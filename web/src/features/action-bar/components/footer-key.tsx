import { Keys } from "@/features/action-bar/components/keys";

/** What some keys do in the action bar, in a word beside them. */
export function FooterKey({ keys, label }: { keys: readonly string[]; label: string }) {
  return (
    <span className="inline-flex items-center gap-1.5">
      <Keys keys={keys} />
      {label}
    </span>
  );
}
