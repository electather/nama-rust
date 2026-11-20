import { Code, ConnectError } from "@connectrpc/connect";
import { fireEvent, screen } from "@testing-library/react";
import { expect, test } from "vite-plus/test";

import {
  type SetSubtitleBackgroundRequest,
  type SetSubtitleSizeRequest,
  SubtitleBackground,
  SubtitleSize,
} from "@/gen/nama/v1/playback_pb";
import { fakeServer, newUser, openApp } from "@/test-app";

test("settings shows the stored size and background, and picking either saves it alone", async () => {
  const appearance: { size: SubtitleSize; background: SubtitleBackground } = {
    size: SubtitleSize.LARGE,
    background: SubtitleBackground.OUTLINE,
  };
  const sized: SetSubtitleSizeRequest[] = [];
  const backed: SetSubtitleBackgroundRequest[] = [];
  const transport = fakeServer({
    signedIn: newUser,
    playbackService: {
      getSubtitleAppearance: () => appearance,
      setSubtitleSize: (request) => {
        sized.push(request);
        appearance.size = request.size;
        return {};
      },
      setSubtitleBackground: (request) => {
        backed.push(request);
        appearance.background = request.background;
        return {};
      },
    },
  });
  openApp("/settings", transport);

  const large = await screen.findByRole<HTMLInputElement>("radio", { name: "Large" });
  const outline = screen.getByRole<HTMLInputElement>("radio", { name: /^Outline/u });
  expect([large.checked, outline.checked]).toStrictEqual([true, true]);
  fireEvent.click(screen.getByRole("radio", { name: "Largest" }));
  fireEvent.click(screen.getByRole("radio", { name: /^Box/u }));

  await expect.poll(() => sized.map(({ size }) => size)).toStrictEqual([SubtitleSize.LARGEST]);
  await expect
    .poll(() => backed.map(({ background }) => background))
    .toStrictEqual([SubtitleBackground.BOX]);
});

test("a size the Server didn't save goes back to the stored one beside the alert", async () => {
  const transport = fakeServer({
    signedIn: newUser,
    playbackService: {
      getSubtitleAppearance: () => ({
        size: SubtitleSize.LARGE,
        background: SubtitleBackground.OUTLINE,
      }),
      setSubtitleSize: () => {
        throw new ConnectError("the database is locked", Code.Internal);
      },
    },
  });
  openApp("/settings", transport);

  const large = await screen.findByRole<HTMLInputElement>("radio", { name: "Large" });
  const small = screen.getByRole<HTMLInputElement>("radio", { name: "Small" });
  fireEvent.click(small);

  await screen.findByText(/^Couldn’t save that/u);
  expect([large.checked, small.checked]).toStrictEqual([true, false]);
});
