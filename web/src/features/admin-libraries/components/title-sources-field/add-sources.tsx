import { Button } from "nama-ui/components/button";

import { ArtworkOnly } from "@/features/admin-libraries/components/title-sources-field/artwork-only";
import { sourceName } from "@/features/admin-libraries/lib/title-sources";
import type { TitleSource } from "@/gen/nama/v1/library_pb";

/** A button per running source the order leaves out, which adds it at the end. */
export function AddSources({
  listed,
  order,
  setOrder,
}: {
  listed: TitleSource[];
  order: string[];
  setOrder: (order: string[]) => void;
}) {
  return (
    <div className="-ms-4 flex flex-wrap gap-1">
      {listed
        .filter((source) => source.running && !order.includes(source.namespace))
        .map((source) => (
          <Button
            key={source.namespace}
            type="button"
            variant="ghost"
            onClick={() => {
              setOrder([...order, source.namespace]);
            }}
          >
            Add {sourceName(source)}
            {!source.searches && <ArtworkOnly />}
          </Button>
        ))}
    </div>
  );
}
