import { create } from "@bufbuild/protobuf";
import { Code, createClient } from "@connectrpc/connect";
import { describe, expect, test } from "vite-plus/test";

import { HubService } from "@/gen/nama/hub/v1/hub_pb";
import { BrowseService, ListMoviesResponseSchema } from "@/gen/nama/v1/browse_pb";
import { PlaybackService, SubtitleBackground } from "@/gen/nama/v1/playback_pb";
import { PluginService } from "@/gen/nama/v1/plugin_pb";
import { ServerService } from "@/gen/nama/v1/server_pb";
import { fakeServer } from "@/test-server";

describe("a method a test overrides", () => {
  const { transport } = fakeServer({}, { PlaybackService: { getAutoplay: () => ({ on: false }) } });
  const playback = createClient(PlaybackService, transport);

  test("answers as the test says", async () => {
    await expect(playback.getAutoplay({})).resolves.toHaveProperty("on", false);
  });

  test("keeps its siblings' answers", async () => {
    await expect(playback.getSubtitleAppearance({})).resolves.toHaveProperty(
      "background",
      SubtitleBackground.OUTLINE,
    );
  });
});

describe("an override that changes the model", () => {
  const { server, transport } = fakeServer(
    {},
    {
      ServerService: {
        updateServerSettings: ({ settings }) => {
          server.serverSettings = settings ?? {};
          return {};
        },
      },
    },
  );
  const client = createClient(ServerService, transport);

  test("changes what later reads answer", async () => {
    await client.updateServerSettings({ settings: { url: "https://media.example" } });

    const read = await client.getServerSettings({});

    expect(read.settings?.url).toBe("https://media.example");
  });
});

describe("a Plugin's one record", () => {
  const tmdb = { summary: { id: "tmdb", name: "TMDB" }, homepageUrl: "https://tmdb.example" };
  const { transport } = fakeServer({ plugins: [tmdb] });
  const plugins = createClient(PluginService, transport);

  test("gives its list row", async () => {
    const listed = await plugins.listPlugins({});

    expect(listed.summaries.map(({ id, name }) => ({ id, name }))).toStrictEqual([tmdb.summary]);
  });

  test("gives its details", async () => {
    const details = await plugins.getPlugin({ id: "tmdb" });

    expect(details.plugin?.homepageUrl).toBe(tmdb.homepageUrl);
  });

  test("is the only one there is", async () => {
    await expect(plugins.getPlugin({ id: "radarr" })).rejects.toHaveProperty("code", Code.NotFound);
  });
});

describe("a method nothing answers", () => {
  test("answers an empty message", async () => {
    const browse = createClient(BrowseService, fakeServer().transport);

    await expect(browse.listMovies({})).resolves.toStrictEqual(create(ListMoviesResponseSchema));
  });

  test("in strict mode, fails naming the method", async () => {
    const browse = createClient(BrowseService, fakeServer({}, {}, { strict: true }).transport);

    await expect(browse.listMovies({})).rejects.toHaveProperty(
      "rawMessage",
      "nothing answers BrowseService.ListMovies",
    );
  });

  test("of the Hub's API is Unimplemented, as at a Server", async () => {
    const hub = createClient(HubService, fakeServer().transport);

    await expect(hub.getHubInfo({})).rejects.toHaveProperty("code", Code.Unimplemented);
  });
});
