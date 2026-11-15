import { useQuery } from "@connectrpc/connect-query";
import { ArrowDown, ArrowUp, X } from "lucide-react";
import { type ReactNode, useEffect, useId, useRef, useState } from "react";

import { Button } from "@/components/ui/button";
import { LibraryService, type TitleSource } from "@/gen/nama/v1/library_pb";

/** A new Library's title sources, as the Server would set them. */
const tmdbAlone = ["tmdb"];

/**
 * The Library's title sources in order, which an Admin reorders, adds to and removes from, sent as
 * one `titleSources` form value per source. At least one always stays.
 */
export function TitleSourcesField({
  initial = tmdbAlone,
  error,
}: {
  initial?: string[] | undefined;
  error?: string | undefined;
}) {
  const id = useId();
  const [order, setOrder, fieldset] = useOrderResetWithForm(initial);
  const { data } = useQuery(LibraryService.method.listTitleSources, {});
  const enabled = data?.titleSources ?? [];

  return (
    <fieldset
      ref={fieldset}
      className="flex flex-col gap-2"
      aria-describedby={`${id}-hint ${id}-error`}
    >
      <legend className="mb-2 text-sm font-medium">Title sources</legend>
      <ol aria-label="Title sources" className="flex flex-col gap-1">
        {order.map((namespace, index) => {
          const source = enabled.find((each) => each.namespace === namespace);
          return (
            <SourceRow
              key={namespace}
              order={order}
              index={index}
              setOrder={setOrder}
              label={source === undefined ? namespace : sourceName(source)}
              // Until the list arrives, no stored source is known to be missing.
              unavailable={data !== undefined && source === undefined}
              artworkOnly={source?.searches === false}
            />
          );
        })}
      </ol>
      <AddSources enabled={enabled} order={order} setOrder={setOrder} />
      <p id={`${id}-hint`} className="text-sm text-muted-foreground">
        Titles are Matched with the first source that finds them. Artwork comes from every source in
        the order.
      </p>
      {/* Always there, so screen readers announce an error as it appears. */}
      <p id={`${id}-error`} role="alert" className="text-sm text-destructive empty:hidden">
        {error}
      </p>
    </fieldset>
  );
}

/**
 * `initial` as state the field changes. The hidden inputs are controlled, so a reset of the form
 * holding the returned fieldset ref (CreateLibrary's, after a create) leaves them be: the order goes back here.
 */
function useOrderResetWithForm(initial: string[]) {
  const [order, setOrder] = useState(initial);
  const fieldset = useRef<HTMLFieldSetElement>(null);
  useEffect(() => {
    const form = fieldset.current?.form;
    const reset = () => {
      setOrder(initial);
    };
    form?.addEventListener("reset", reset);
    return () => form?.removeEventListener("reset", reset);
  }, [initial]);
  return [order, setOrder, fieldset] as const;
}

/** A button per enabled source the order leaves out, which adds it at the end. */
function AddSources({
  enabled,
  order,
  setOrder,
}: {
  enabled: TitleSource[];
  order: string[];
  setOrder: (order: string[]) => void;
}) {
  return (
    <div className="-ms-4 flex flex-wrap gap-1">
      {enabled
        .filter((source) => !order.includes(source.namespace))
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

/** The note beside a source that only adds artwork, as it can't search for titles to Match. */
function ArtworkOnly() {
  return <span className="text-muted-foreground"> · Artwork only</span>;
}

/** What people know `source` by: its Plugin's name, or else its namespace. */
function sourceName({ namespace, pluginName }: TitleSource) {
  return pluginName === "" ? namespace : pluginName;
}

/** The title source at `index` in `order`, with the buttons that move it or take it out. */
function SourceRow({
  order,
  index,
  setOrder,
  label,
  unavailable,
  artworkOnly,
}: {
  order: string[];
  index: number;
  setOrder: (order: string[]) => void;
  label: string;
  unavailable: boolean;
  artworkOnly: boolean;
}) {
  const move = (by: -1 | 1) => () => {
    const next = [...order];
    next.splice(index + by, 0, ...next.splice(index, 1));
    setOrder(next);
  };
  return (
    <li className="flex items-center gap-1 text-sm">
      <input type="hidden" name="titleSources" value={order[index]} />
      <span className="me-auto">
        {label}
        {unavailable && <span className="text-muted-foreground"> · not available</span>}
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

/** An icon button on a title source's row, named by `name`. */
function RowButton({
  name,
  disabled,
  onClick,
  children,
}: {
  name: string;
  disabled: boolean;
  onClick: () => void;
  children: ReactNode;
}) {
  return (
    <Button
      type="button"
      variant="ghost"
      size="icon"
      aria-label={name}
      disabled={disabled}
      onClick={onClick}
    >
      {children}
    </Button>
  );
}
