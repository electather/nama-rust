import { sourceName, sourceNote } from "@/features/admin-libraries/lib/title-sources";
import type { TitleSource } from "@/gen/nama/v1/library_pb";
import { OrderList } from "@/shared/components/home-layout/order-list";

/**
 * The title sources in `order`, reordered or taken out as Home's rows are, each change handed to
 * `setOrder`, the last one kept. `loaded` says the Server's `listed` sources have arrived.
 */
export function SourceList({
  order,
  setOrder,
  listed,
  loaded,
}: {
  order: string[];
  setOrder: (order: string[]) => void;
  listed: TitleSource[];
  loaded: boolean;
}) {
  return (
    <OrderList
      label="Title sources"
      order={order}
      setOrder={setOrder}
      keepOne
      nameOf={(namespace) => {
        const source = listed.find((each) => each.namespace === namespace);
        return {
          label: source === undefined ? namespace : sourceName(source),
          from: sourceNote(source, loaded),
        };
      }}
    />
  );
}
