import { screen } from "@testing-library/react";
import { expect, test } from "vite-plus/test";

import type { UpdatePlaybackSettingsRequest } from "@/gen/nama/v1/server_pb";
import { admin, fakeServer } from "@/test-server";
import { openSettings, pick } from "@/test-settings";

test("space for subtitles starts at 5 GB, says how much the cache takes now, and saves the space picked", async () => {
  const updates: UpdatePlaybackSettingsRequest[] = [];
  openSettings(
    "server-playback",
    fakeServer(
      { signedIn: admin },
      {
        ServerService: {
          getPlaybackStatus: () => ({}),
          getPlaybackSettings: () => ({ settings: {}, subtitleCacheBytes: 1_500_000_000n }),
          updatePlaybackSettings: (request) => {
            updates.push(request);
            return { settings: request.settings };
          },
        },
      },
    ).transport,
  );

  const space = await screen.findByRole("combobox", { name: "Space for subtitles" });
  expect(space.textContent).toBe("5 GB");
  expect(screen.queryByText(/1\.5 GB in use now\.$/u)).not.toBeNull();
  pick("Space for subtitles", "10 GB");

  await expect.poll(() => updates).toMatchObject([{ settings: { subtitleCacheMib: 10_240 } }]);
});
