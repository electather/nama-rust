import { useBoxSetCount } from "@/features/library/data/use-box-set-count";
import { countLine } from "@/features/library/lib/count-line";

/** "12 Box Sets": how many a Movies Library has. */
export function BoxSetCountLine({ libraryId }: { libraryId: bigint }) {
  const count = useBoxSetCount(libraryId);
  return countLine({ shown: count, all: count }, { one: "Box Set", many: "Box Sets" });
}
