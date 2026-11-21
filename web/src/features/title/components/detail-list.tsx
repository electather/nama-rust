import { DetailFact } from "@/features/title/components/detail-fact";
import type { DetailGroup } from "@/features/title/lib/detail-group";

/** One group of a title's Details: its heading over its facts. */
export function DetailList({ heading, facts }: DetailGroup) {
  return (
    <div>
      <h2 className="mb-3.5 text-lg font-semibold">{heading}</h2>
      <dl className="flex flex-col gap-3.5">
        {facts.map(([term, value]) => (
          <DetailFact key={term} term={term} value={value} />
        ))}
      </dl>
    </div>
  );
}
