import type { MessageInitShape } from "@bufbuild/protobuf";
import { cleanup, screen, waitFor, within } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import type { SubscribeResponse } from "@/gen/nama/v1/event_pb";
import {
  type DownloadSchema,
  DownloadStatus,
  RequestService,
  RequestState,
} from "@/gen/nama/v1/request_pb";
import { eventFeed, fakeServer, newUser, openApp } from "@/test-app";

afterEach(cleanup);

type Download = MessageInitShape<typeof DownloadSchema>;

/** Dune, Processing, asked for by Grace, with `downloads` as its Fulfiller last reported them. */
function duneDownloading(downloads: Download[]) {
  return {
    id: 1n,
    movieOrSeason: {
      case: "movie" as const,
      value: { id: { namespace: "tmdb", value: "438631" }, title: "Dune" },
    },
    state: RequestState.PROCESSING,
    requesters: [{ userId: 3n, name: "Grace" }],
    downloads,
  };
}

/** A fake Server listing the Requests `requests` gives, and sending `events`. */
function server(
  requests: () => ReturnType<typeof duneDownloading>[],
  events = new ReadableStream<SubscribeResponse>(),
) {
  return fakeServer({ signedIn: newUser, events }, {}, {}, (router) => {
    router.service(RequestService, { listRequests: () => ({ requests: requests() }) });
  });
}

test("a handed-off Request shows the Quality its service used, else the profile it kept", async () => {
  const asked4k = { ...duneDownloading([]), quality: { id: 7n, name: "4K", profileId: "5" } };
  const usedHd = { ...asked4k, qualityUsed: { id: 6n, name: "HD", profileId: "4" } };
  const keptItsOwn = { ...asked4k, id: 2n, qualityUsed: { name: "Ultra-HD", profileId: "9" } };
  openApp(
    "/?settings=requests",
    server(() => [usedHd, keptItsOwn]),
  );

  const rows = await within(
    await screen.findByRole("list", { name: /^(?:Your|All) Requests$/u }),
  ).findAllByRole("listitem");
  expect(rows.map((row) => row.textContent)).toStrictEqual([
    "DuneProcessingHD · Requested by Grace",
    "DuneProcessingUltra-HD (set in the service) · Requested by Grace",
  ]);
});

const duneFile = {
  title: "Dune.2021.2160p",
  sizeBytes: 4_000_000_000n,
  etaSeconds: 5400n,
  status: DownloadStatus.DOWNLOADING,
};

test("a Processing Request's bar moves without a reload as its download progresses", async () => {
  const events = eventFeed();
  let request = duneDownloading([{ ...duneFile, percent: 20 }]);
  openApp(
    "/?settings=requests",
    server(() => [request], events.stream),
  );
  const bar = await screen.findByRole("progressbar", { name: "Downloaded" });
  expect(bar.getAttribute("aria-valuenow")).toBe("20");

  request = duneDownloading([{ ...duneFile, percent: 65 }]);
  events.send({ event: { case: "requestProgress", value: { request } } });

  await waitFor(() => {
    const moved = screen.getByRole("progressbar", { name: "Downloaded" });
    expect(moved.getAttribute("aria-valuenow")).toBe("65");
  });
});

test("a Processing Request's download that needs attention says why, in the Fulfiller's words", async () => {
  openApp(
    "/?settings=requests",
    server(() => [
      duneDownloading([
        { ...duneFile, percent: 100, status: DownloadStatus.IMPORTING },
        {
          title: "Dune.2021.Extras",
          percent: 30,
          status: DownloadStatus.NEEDS_ATTENTION,
          attentionReason: "Radarr said: Automatic import is not possible.",
        },
      ]),
    ]),
  );

  await expect(
    screen.findByText(
      "Dune.2021.Extras stopped, and whoever runs the Server needs to look at it. Radarr said: Automatic import is not possible.",
    ),
  ).resolves.toBeDefined();
  // Weighted by size where known, else counted evenly: (100 + 30) / 2.
  const bar = screen.getByRole("progressbar", { name: "Downloaded" });
  expect(bar.getAttribute("aria-valuenow")).toBe("65");
});
