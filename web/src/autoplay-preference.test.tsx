import { fireEvent, screen } from "@testing-library/react";
import { expect, test } from "vite-plus/test";

import type { SetAutoplayRequest } from "@/gen/nama/v1/playback_pb";

import { fakeServer, newUser, openApp } from "./test-app";

test("the autoplay switch shows the stored value, and toggling it saves the other", async () => {
  let on = false;
  const set: SetAutoplayRequest[] = [];
  const transport = fakeServer({
    signedIn: newUser,
    playbackService: {
      getAutoplay: () => ({ on }),
      setAutoplay: (request) => {
        set.push(request);
        ({ on } = request);
        return {};
      },
    },
  });
  openApp("/settings", transport);

  const autoplay = await screen.findByRole<HTMLInputElement>("switch", {
    name: "Play the next Episode automatically",
  });
  expect(autoplay.checked).toBe(false);
  fireEvent.click(autoplay);

  await expect.poll(() => set).toMatchObject([{ on: true }]);
  await expect.poll(() => autoplay.checked).toBe(true);
});
