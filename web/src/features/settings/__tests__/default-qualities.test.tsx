import type { MessageInitShape } from "@bufbuild/protobuf";
import { createConnectQueryKey } from "@connectrpc/connect-query";
import { act, cleanup, fireEvent, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { ExtensionPoint, type QualitySchema } from "@/gen/nama/v1/plugin_pb";
import { RequestService, type SetDefaultQualityRequest } from "@/gen/nama/v1/request_pb";
import { Permission } from "@/gen/nama/v1/sign_in_pb";
import { fakeServer, fill, newUser, openApp } from "@/test-app";

afterEach(cleanup);

const mayChoose = [...newUser, Permission.CHOOSE_QUALITY];

const hd = { id: 1n, profileId: "4", name: "HD", isDefault: true };
const fourK = { id: 2n, profileId: "5", name: "4K" };

/** A fake Server whose Movie Fulfiller is Radarr, where the signed-in User's default is `movies`. */
function server(
  permissions: Permission[],
  movies: { qualities?: MessageInitShape<typeof QualitySchema>[]; quality?: bigint },
  set: SetDefaultQualityRequest[] = [],
) {
  const radarr = {
    pluginId: "radarr",
    fulfiller: ExtensionPoint.MOVIE_FULFILLER,
    qualities: [hd, fourK],
    ...movies,
  };
  return fakeServer({ signedIn: permissions }, {}, {}, (router) => {
    router.service(RequestService, {
      listDefaultQualities: () => ({ defaults: [radarr] }),
      setDefaultQuality: (request) => {
        set.push(request);
        return {};
      },
    });
  });
}

test("a User who may choose sets their default Quality for Movies, the default picked to start with", async () => {
  const set: SetDefaultQualityRequest[] = [];
  openApp("/settings", server(mayChoose, {}, set));

  const movies = await screen.findByLabelText<HTMLSelectElement>("Movies");
  expect(movies.value).toBe("1");
  expect(screen.getByRole("option", { name: "HD (Default)" })).toBeDefined();
  fill("Movies", "2");
  fireEvent.click(screen.getByRole("button", { name: "Save default for Movies" }));

  await expect(screen.findByText("Saved.")).resolves.toBeDefined();
  expect(set).toMatchObject([{ pluginId: "radarr", quality: 2n }]);
});

test("a default an admin has since removed is flagged, with the default picked", async () => {
  openApp("/settings", server(mayChoose, { qualities: [hd], quality: 2n }));

  const movies = await screen.findByLabelText<HTMLSelectElement>("Movies");
  expect(movies.value).toBe("1");
  expect(movies.getAttribute("aria-invalid")).toBe("true");
  expect(
    screen.getByText("The Quality you picked is no longer offered, so your Requests get HD."),
  ).toBeDefined();
});

test("a User who may not choose a Quality sees no default Quality on Settings", async () => {
  openApp("/settings", server(newUser, { quality: 2n }));

  await screen.findByRole("heading", { name: "Devices" });
  await waitFor(() => {
    expect(screen.queryByRole("heading", { name: "Your default Quality" })).toBeNull();
  });
  expect(screen.queryByLabelText("Movies")).toBeNull();
});

test("saving a default for Shows reads the Show Quality menu's Qualities again", async () => {
  let personalDefault: bigint | undefined;
  const sonarr = {
    pluginId: "sonarr",
    fulfiller: ExtensionPoint.SHOW_FULFILLER,
    qualities: [hd, fourK],
  };
  const dark = {
    id: { namespace: "tmdb", value: "70523" },
    title: "Dark",
    seasons: [{ seasonNumber: 1 }],
  };
  const transport = fakeServer({ signedIn: mayChoose }, {}, {}, (router) => {
    router.service(RequestService, {
      listRequests: () => ({}),
      lookUpShow: () => ({ show: dark }),
      listShowQualities: () => ({ qualities: [hd, fourK], personalDefault }),
      listDefaultQualities: () => ({ defaults: [{ ...sonarr, quality: personalDefault }] }),
      setDefaultQuality: ({ quality }) => {
        personalDefault = quality;
        return {};
      },
    });
  });
  const router = openApp("/shows/tmdb-70523", transport);
  const chevron = "Request Seasons of Dark at a Quality";
  await waitFor(() => {
    expect(screen.getByRole<HTMLButtonElement>("button", { name: chevron }).disabled).toBe(false);
  });

  await act(async () => {
    await router.navigate({ to: "/settings" });
  });
  await screen.findByLabelText("Shows");
  fill("Shows", "2");
  fireEvent.click(screen.getByRole("button", { name: "Save default for Shows" }));
  await screen.findByText("Saved.");

  const showQualitiesKey = createConnectQueryKey({
    schema: RequestService.method.listShowQualities,
    cardinality: "finite",
  });
  // Unfixed, the menu below still shows the new default if a slow run remounts the Show page over
  // 1 s later: useSuspenseQuery keeps data fresh for at least 1 s, then refetches on mount.
  const cached = router.options.context.queryClient
    .getQueryCache()
    .findAll({ queryKey: showQualitiesKey });
  expect(cached.map((query) => query.state.isInvalidated)).toStrictEqual([true]);
  await act(async () => {
    await router.navigate({ to: "/shows/$showId", params: { showId: "tmdb-70523" } });
  });
  await waitFor(() => {
    expect(screen.getByRole<HTMLButtonElement>("button", { name: chevron }).disabled).toBe(false);
  });
  fireEvent.click(screen.getByRole("button", { name: chevron }));
  const items = await screen.findAllByRole("menuitemradio");
  expect(items.map((item) => item.getAttribute("aria-checked"))).toStrictEqual(["false", "true"]);
});
