import type { MessageInitShape } from "@bufbuild/protobuf";
import { act, cleanup, fireEvent, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { ExtensionPoint, type QualitySchema } from "@/gen/nama/v1/plugin_pb";
import { RequestService, type SetDefaultQualityRequest } from "@/gen/nama/v1/request_pb";
import { Permission } from "@/gen/nama/v1/sign_in_pb";
import { fakeServer, newUser, openApp } from "@/test-app";
import { openSettings, pick } from "@/test-settings";

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
      listRequests: () => ({}),
      setDefaultQuality: (request) => {
        set.push(request);
        return {};
      },
    });
  });
}

test("a User who may choose sets their default Quality for Movies, the default picked to start with", async () => {
  const set: SetDefaultQualityRequest[] = [];
  openSettings("requests", server(mayChoose, {}, set));

  const movies = await screen.findByRole("combobox", { name: "Movies" });
  expect(movies.textContent).toBe("HD (Default)");
  pick("Movies", "4K");

  await expect.poll(() => set).toMatchObject([{ pluginId: "radarr", quality: 2n }]);
});

test("a default an admin has since removed is flagged, with the default picked", async () => {
  openSettings("requests", server(mayChoose, { qualities: [hd], quality: 2n }));

  const movies = await screen.findByRole("combobox", { name: "Movies" });
  expect(movies.textContent).toBe("HD (Default)");
  expect(
    screen.getByText("The Quality you picked is no longer offered, so your Requests get HD."),
  ).toBeDefined();
});

test("a User who may not choose a Quality sees their Requests without a default Quality", async () => {
  openSettings("requests", server(newUser, { quality: 2n }));

  await screen.findByText("No Requests yet. Request a Movie or Show from its page.");
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
    await router.navigate({ to: ".", search: { settings: "requests" } });
  });
  await screen.findByRole("combobox", { name: "Shows" });
  pick("Shows", "4K");
  await expect.poll(() => personalDefault).toBe(2n);

  // Settings sits over the Show's page, so closing it is all it takes to be back there.
  fireEvent.click(screen.getByRole("button", { name: "Close Settings" }));
  await waitFor(() => {
    expect(router.state.location.search.settings).toBeUndefined();
  });
  fireEvent.click(screen.getByRole("button", { name: chevron }));
  const items = await screen.findAllByRole("menuitemradio");
  expect(items.map((item) => item.getAttribute("aria-checked"))).toStrictEqual(["false", "true"]);
});
