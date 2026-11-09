import { createConnectQueryKey, useMutation, useQuery } from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";

import { FormEnd, FormSelect } from "@/components/form-field";
import { ExtensionPoint, type Quality } from "@/gen/nama/v1/plugin_pb";
import { type DefaultQuality, RequestService } from "@/gen/nama/v1/request_pb";
import { formText, onSubmitted, refusal } from "@/lib/forms";

const defaultsKey = createConnectQueryKey({
  schema: RequestService.method.listDefaultQualities,
  cardinality: "finite",
});

const movieQualitiesKey = createConnectQueryKey({
  schema: RequestService.method.listMovieQualities,
  cardinality: "finite",
});

/** Each Fulfiller's field label: what it fetches. */
const fulfillerLabels: Partial<Record<ExtensionPoint, string>> = {
  [ExtensionPoint.MOVIE_FULFILLER]: "Movies",
  [ExtensionPoint.SHOW_FULFILLER]: "Shows",
};

/**
 * The signed-in User's own default Quality for each Fulfiller with more than one, which their
 * Requests get when they don't pick. For a User who may choose a Quality; nothing while no
 * Fulfiller has anything to pick or flag.
 */
export function DefaultQualities() {
  const { data } = useQuery(RequestService.method.listDefaultQualities, {});
  if (!data || data.defaults.length === 0) return null;
  return (
    <section aria-labelledby="default-quality" className="mt-12 max-w-sm">
      <h2 id="default-quality" className="font-medium">
        Your default Quality
      </h2>
      <p className="mt-1 text-sm text-muted-foreground">
        What your Requests are downloaded at when you don’t pick a Quality.
      </p>
      {data.defaults.map((fulfiller) => (
        <DefaultQualityForm key={fulfiller.pluginId} fulfiller={fulfiller} />
      ))}
    </section>
  );
}

/** The form a User sets their default from one Fulfiller's Qualities with. */
function DefaultQualityForm({ fulfiller }: { fulfiller: DefaultQuality }) {
  const { pluginId } = fulfiller;
  const kind = fulfillerLabels[fulfiller.fulfiller] ?? pluginId;
  const save = useSaveDefault();
  return (
    <form
      className="mt-4 flex flex-col gap-5"
      onSubmit={onSubmitted((typed) => {
        save.mutate({ pluginId, quality: BigInt(formText(typed, "quality")) });
      })}
    >
      <QualitySelect label={kind} fulfiller={fulfiller} />
      <FormEnd
        action={`Save default for ${kind}`}
        variant="ghost"
        pending={save.isPending}
        done={save.isSuccess ? "Saved." : undefined}
        refusal={
          save.error ? refusal(save.error, `save your default for ${kind}`, {}).form : undefined
        }
      />
    </form>
  );
}

/**
 * A dropdown of the Fulfiller's Qualities, starting at the User's default, else the Fulfiller's,
 * which is marked. Flags a default an admin has since removed, as the Fulfiller's applies then.
 */
function QualitySelect({
  label,
  fulfiller: { qualities, quality },
}: {
  label: string;
  fulfiller: DefaultQuality;
}) {
  const listDefault = qualities.find((offered) => offered.isDefault);
  const yours = qualities.find((offered) => offered.id === quality);
  return (
    <FormSelect
      label={label}
      name="quality"
      defaultValue={String((yours ?? listDefault)?.id)}
      error={removedFlag(quality, yours, listDefault)}
    >
      {qualities.map((offered) => (
        <option key={String(offered.id)} value={String(offered.id)}>
          {marked(offered)}
        </option>
      ))}
    </FormSelect>
  );
}

/** `quality`'s name, marked if it's the Fulfiller's default. */
function marked(quality: Quality) {
  return quality.isDefault ? `${quality.name} (Default)` : quality.name;
}

/**
 * Why the User's default `quality` no longer applies, if `yours`, its Quality on the list, is gone,
 * and that `listDefault` does instead.
 */
function removedFlag(
  quality: bigint | undefined,
  yours: Quality | undefined,
  listDefault: Quality | undefined,
) {
  const removed = quality !== undefined && yours === undefined;
  return removed
    ? `The Quality you picked is no longer offered, so your Requests get ${listDefault?.name}.`
    : undefined;
}

/** Saving a default Quality, then reading the defaults and the Request dialog's Qualities again. */
function useSaveDefault() {
  const queryClient = useQueryClient();
  return useMutation(RequestService.method.setDefaultQuality, {
    onSuccess: () =>
      Promise.all([
        queryClient.invalidateQueries({ queryKey: defaultsKey }),
        queryClient.invalidateQueries({ queryKey: movieQualitiesKey }),
      ]),
  });
}
