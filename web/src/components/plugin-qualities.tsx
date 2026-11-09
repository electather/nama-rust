import { create } from "@bufbuild/protobuf";
import { useMutation, useQuery } from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";
import { useState } from "react";

import { FormEnd } from "@/components/form-field";
import { AddQuality, QualityFields } from "@/components/quality-fields";
import {
  type Choice,
  ExtensionPoint,
  type Plugin,
  PluginService,
  type Quality,
  QualitySchema,
  SettingKind,
} from "@/gen/nama/v1/plugin_pb";
import { RefusalReason } from "@/gen/nama/v1/refusal_pb";
import { formText, onSubmitted, refusal, refusalOf, serverMessage } from "@/lib/forms";
import { pluginName } from "@/lib/plugins";

/** The choice setting whose choices a Fulfiller's Qualities offer (`qualities.rs` on the Server). */
const qualityProfile = "quality_profile";

/**
 * Whether `plugin` takes Qualities: a Movie Fulfiller with the `quality_profile` choice setting.
 * Shows have none yet.
 */
function takesQualities(plugin: Plugin) {
  return (
    plugin.extensionPoints.includes(ExtensionPoint.MOVIE_FULFILLER) &&
    plugin.settings.some(({ key, kind }) => key === qualityProfile && kind === SettingKind.CHOICE)
  );
}

/** A Quality being edited, keyed so its inputs keep their names as others are removed. */
interface Row {
  key: number;
  quality: Quality;
}

let nextKey = 0;

function keyed(qualities: Quality[]): Row[] {
  return qualities.map((quality) => ({ key: nextKey++, quality }));
}

/**
 * The Qualities an Admin offers Requesters on a Fulfiller's page: the service's quality profiles,
 * as its settings check offers them on the stored settings, under friendly names. Nothing for a
 * Plugin that takes none.
 */
export function PluginQualities({ plugin }: { plugin: Plugin }) {
  return takesQualities(plugin) && <QualitiesSection plugin={plugin} />;
}

function QualitiesSection({ plugin }: { plugin: Plugin }) {
  const listed = useQuery(PluginService.method.listQualities, { id: plugin.id });
  return (
    <section aria-labelledby="qualities" className="mt-10">
      <h2 id="qualities" className="font-medium">
        Qualities
      </h2>
      <p className="mt-1 max-w-prose text-sm text-pretty text-muted-foreground">
        What Requests are downloaded at: {pluginName(plugin)}’s quality profiles under names you
        choose. A new Request gets the default. With none, the Quality profile setting applies.
      </p>
      {listed.data ? (
        <QualitiesForm plugin={plugin} saved={listed.data.qualities} />
      ) : (
        <p role="alert" className="mt-4 text-sm text-destructive empty:hidden">
          {listed.error && "Couldn’t load the Qualities. Check that the Server is running."}
        </p>
      )}
    </section>
  );
}

function QualitiesForm({ plugin, saved }: { plugin: Plugin; saved: Quality[] }) {
  const [rows, setRows] = useState(() => keyed(saved));
  const queryClient = useQueryClient();
  const replace = useMutation(PluginService.method.replaceQualities, {
    onSuccess: async ({ qualities }) => {
      setRows(keyed(qualities));
      await queryClient.invalidateQueries();
    },
  });
  const { profiles, unoffered, gone } = useProfiles(plugin, rows);
  const errors = { ...gone, ...(replace.error && qualityErrors(replace.error, rows, plugin)) };
  return (
    <form
      className="mt-4 flex max-w-md flex-col gap-5"
      onSubmit={onSubmitted((typed) => {
        replace.mutate({ id: plugin.id, qualities: qualitiesTyped(typed, rows) });
      })}
    >
      <p role="alert" className="text-sm text-destructive empty:hidden">
        {unoffered}
      </p>
      {rows.map((row, index) => (
        <QualityFields
          key={row.key}
          rowKey={row.key}
          quality={row.quality}
          number={index + 1}
          profiles={profiles}
          errors={errors}
          onRemove={() => {
            setRows(rows.filter((each) => each.key !== row.key));
          }}
        />
      ))}
      <AddQuality
        onAdd={() => {
          const quality = create(QualitySchema, { isDefault: rows.length === 0 });
          setRows([...rows, ...keyed([quality])]);
        }}
      />
      <FormEnd
        action={replace.isPending ? "Saving…" : "Save Qualities"}
        variant="ghost"
        pending={replace.isPending}
        done={replace.isSuccess ? "Saved. New Requests get the default." : undefined}
        refusal={errors["form"]}
      />
    </form>
  );
}

/**
 * The profiles `plugin`'s settings check offers on its stored settings, why there are none when it
 * failed, and a flag under each of `rows` whose profile it doesn't offer. A secret isn't sent, so
 * the Server uses the stored one.
 */
function useProfiles(plugin: Plugin, rows: Row[]) {
  const stored = Object.fromEntries(
    plugin.settings.flatMap(({ key, value }) => (value ? [[key, value]] : [])),
  );
  const offered = useQuery(PluginService.method.testPluginSettings, {
    id: plugin.id,
    settings: stored,
  });
  const profiles = offered.data?.choices[qualityProfile]?.choices;
  return {
    profiles: profiles ?? [],
    gone: profilesGone(rows, profiles, plugin),
    unoffered:
      offered.error &&
      `Couldn’t get ${pluginName(plugin)}’s quality profiles: ${serverMessage(offered.error)}`,
  };
}

/** A flag under each of `rows` whose saved profile `plugin` doesn't offer now, as `profiles` says. */
function profilesGone(rows: Row[], profiles: Choice[] | undefined, plugin: Plugin) {
  if (profiles === undefined) return {};
  const gone = rows.filter(
    ({ quality: { profileId } }) =>
      profileId !== "" && !profiles.some(({ value }) => value === profileId),
  );
  return Object.fromEntries(gone.map(({ key }) => [`profile-${key}`, notOffered(plugin)]));
}

/** The Qualities typed in `typed`, one per row, in order. */
function qualitiesTyped(typed: FormData, rows: Row[]) {
  return rows.map(({ key, quality }) => ({
    id: quality.id,
    profileId: formText(typed, `profile-${key}`),
    name: formText(typed, `name-${key}`),
    description: formText(typed, `description-${key}`),
    isDefault: formText(typed, "default") === String(key),
  }));
}

/** Why the Server refused the Qualities: next to the input its Refusal names, or for the form. */
function qualityErrors(
  error: unknown,
  rows: Row[],
  plugin: Plugin,
): Record<string, string | undefined> {
  const { reason, field } = refusalOf(error);
  const named = refusedInput(field, rows);
  if (named === undefined) return { form: formRefused(error) };
  const [row, input] = named;
  return { [`${input}-${row.key}`]: inputRefused(plugin)[input][reason] ?? serverMessage(error) };
}

/** The row and input a Refusal's `field`, such as `qualities[1].name`, names, if it names one. */
function refusedInput(field: string, rows: Row[]) {
  const [, index, part] = /^qualities\[(\d+)\]\.(profile_id|name)$/u.exec(field) ?? [];
  const row = rows[Number(index)];
  return row && ([row, part === "name" ? "name" : "profile"] as const);
}

/** Why the Server refused the Qualities as a whole. */
function formRefused(error: unknown) {
  return refusal(
    error,
    "save the Qualities",
    {},
    {
      [RefusalReason.DEFAULT_QUALITY_REQUIRED]: { form: "Mark one Quality as the default." },
      // The Plugin's own words.
      [RefusalReason.SETTINGS_CHECK_FAILED]: { form: serverMessage(error) },
    },
  ).form;
}

/** Why the Server refused a Quality's name or profile, by its Refusal's reason. */
function inputRefused(
  plugin: Plugin,
): Record<"name" | "profile", Partial<Record<RefusalReason, string>>> {
  return {
    name: { [RefusalReason.QUALITY_REPEATED]: "Another Quality has this name." },
    profile: {
      [RefusalReason.QUALITY_REPEATED]: "Another Quality offers this profile.",
      [RefusalReason.CHOICE_NOT_OFFERED]: notOffered(plugin),
    },
  };
}

function notOffered(plugin: Plugin) {
  return `${pluginName(plugin)} doesn’t offer this profile now.`;
}
