import { ArrowDown, ArrowUp, X } from "lucide-react";

import { ArtworkOnly } from "@/features/admin-libraries/components/title-sources-field/artwork-only";
import { RowButton } from "@/features/admin-libraries/components/title-sources-field/row-button";

/** The title source at `index` in `order`, with the buttons that move it or take it out. */
export function SourceRow({
  order,
  index,
  setOrder,
  label,
  note,
  artworkOnly,
}: {
  order: string[];
  index: number;
  setOrder: (order: string[]) => void;
  label: string;
  note: string | undefined;
  artworkOnly: boolean;
}) {
  const move = (by: -1 | 1) => () => {
    const next = [...order];
    next.splice(index + by, 0, ...next.splice(index, 1));
    setOrder(next);
  };
  return (
    <li className="flex items-center gap-1 text-sm">
      <span className="me-auto">
        {label}
        {note !== undefined && <span className="text-muted-foreground"> · {note}</span>}
        {artworkOnly && <ArtworkOnly />}
      </span>
      <RowButton name={`Move ${label} up`} disabled={index === 0} onClick={move(-1)}>
        <ArrowUp aria-hidden />
      </RowButton>
      <RowButton
        name={`Move ${label} down`}
        disabled={index === order.length - 1}
        onClick={move(1)}
      >
        <ArrowDown aria-hidden />
      </RowButton>
      <RowButton
        name={`Remove ${label}`}
        disabled={order.length === 1}
        onClick={() => {
          setOrder(order.filter((_, each) => each !== index));
        }}
      >
        <X aria-hidden />
      </RowButton>
    </li>
  );
}
