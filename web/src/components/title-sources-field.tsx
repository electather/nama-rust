import { useQuery } from "@connectrpc/connect-query";
import { Link, useRouteContext } from "@tanstack/react-router";
import { ArrowDown, ArrowUp, X } from "lucide-react";
import { type ReactNode, useEffect, useId, useRef, useState } from "react";

import { LibraryService, type TitleSource } from "@/gen/nama/v1/library_pb";
import { Permission } from "@/gen/nama/v1/sign_in_pb";
import { may } from "@/shared/lib/permissions";
import { Button } from "@/shared/ui/button";

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
  const listed = data?.titleSources ?? [];

  return (
    <fieldset
      ref={fieldset}
      className="flex flex-col gap-2"
      aria-describedby={`${id}-hint ${id}-error`}
    >
      <legend className="mb-2 text-sm font-medium">Title sources</legend>
      <ol aria-label="Title sources" className="flex flex-col gap-1">
        {order.map((namespace, index) => {
          const source = listed.find((each) => each.namespace === namespace);
          return (
            <SourceRow
              key={namespace}
              order={order}
              index={index}
              setOrder={setOrder}
              label={source === undefined ? namespace : sourceName(source)}
              // Until the list arrives, no stored source is known to be missing.
              note={data === undefined ? undefined : unusable(source)}
              artworkOnly={source?.searches === false}
            />
          );
        })}
      </ol>
      <AddSources listed={listed} order={order} setOrder={setOrder} />
      <p id={`${id}-hint`} className="text-sm text-muted-foreground">
        Titles are Matched with the first source that finds them. Artwork comes from every source in
        the order.
      </p>
      {data !== undefined && <NoSourceSearches order={order} listed={listed} />}
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

/** A button per running source the order leaves out, which adds it at the end. */
function AddSources({
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

/**
 * Why the Library's Items wait to be Matched, if no source in `order` is running and searches:
 * each source in turn, and whether it isn't running or only adds artwork. Links to the Plugins
 * only for a User who may enable one there.
 */
function NoSourceSearches({ order, listed }: { order: string[]; listed: TitleSource[] }) {
  const { signedIn } = useRouteContext({ from: "/_member" });
  const sources = order.map((namespace) => ({
    namespace,
    source: listed.find((each) => each.namespace === namespace),
  }));
  if (sources.some(({ source }) => source?.running === true && source.searches)) {
    return null;
  }
  const why = sources.map(({ namespace, source }) => {
    if (source === undefined) {
      return `${namespace} isn’t running`;
    }
    return source.running
      ? `${sourceName(source)} only adds artwork`
      : `${sourceName(source)} isn’t running`;
  });
  const plugins = may(signedIn.user, Permission.MANAGE_SETTINGS) ? (
    <Link
      to="/admin/plugins"
      className="rounded-sm underline decoration-muted-foreground underline-offset-4 outline-none hover:decoration-foreground focus-visible:ring-3 focus-visible:ring-ring/50"
    >
      Plugins
    </Link>
  ) : (
    "Plugins"
  );
  return (
    <p className="text-sm text-pretty">
      No source above can search for titles: {names.format(why)}. The Library’s Items wait to be
      Matched until one can. Enable or set up a Plugin that searches under {plugins}, or add one
      here.
    </p>
  );
}

const names = new Intl.ListFormat("en-GB", { type: "conjunction" });

/** The note beside a source that only adds artwork, as it can't search for titles to Match. */
function ArtworkOnly() {
  return <span className="text-muted-foreground"> · Artwork only</span>;
}

/** Why a source in the order can't be used now: no listed source keeps it, or it isn't running. */
function unusable(source: TitleSource | undefined) {
  if (source === undefined) {
    return "not available";
  }
  return source.running ? undefined : "not running";
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
      <input type="hidden" name="titleSources" value={order[index]} />
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
