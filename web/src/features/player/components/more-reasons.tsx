import { Reasons } from "@/features/player/components/reasons";

/** The reasons that are only context, folded away until opened. */
export function MoreReasons({ texts }: { texts: string[] }) {
  if (texts.length === 0) {
    return null;
  }
  return (
    <details className="mt-2">
      <summary className="cursor-pointer">More</summary>
      <Reasons texts={texts} />
    </details>
  );
}
