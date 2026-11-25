import { create } from "@bufbuild/protobuf";
import { cleanup, fireEvent, screen, within } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { LinkedServerSchema, LinkedUserSchema } from "@/gen/nama/hub/v1/link_pb";
import { openApp } from "@/test-app";
import { accountHub } from "@/test-hub";

afterEach(cleanup);

const cabin = create(LinkedServerSchema, {
  id: "0193-cabin",
  name: "Cabin",
  url: "https://cabin.example",
  linkedAt: 1_700_000_000n,
});
const basement = create(LinkedServerSchema, { id: "0193-basement", name: "Basement" });
const grace = create(LinkedUserSchema, { id: "0199-grace", name: "Grace", serverName: "Cabin" });

/** The Servers Section's group titled `title`. */
async function group(title: string) {
  const heading = await screen.findByRole("heading", { name: title });
  return within(heading.closest("[data-setting]") ?? document.body);
}

test("at the Hub, Settings lists the Account's Servers, each opening at its Server URL if it has one", async () => {
  openApp("/?settings=account-servers", accountHub({ servers: [cabin, basement] }).transport);

  const servers = await group("Your Servers");
  const link = await servers.findByRole("link", { name: "Cabin" });
  expect(link.getAttribute("href")).toBe("https://cabin.example");
  expect(servers.getByText("Basement")).toBeDefined();
  expect(servers.queryByRole("link", { name: "Basement" })).toBeNull();
});

test("unlinking a User takes them off the Account's Users", async () => {
  const { hub, transport } = accountHub({ servers: [cabin], users: [grace] });
  openApp("/?settings=account-servers", transport);

  const users = await group("Your Users");
  fireEvent.click(await users.findByRole("button", { name: "Unlink Grace on Cabin" }));

  await expect(users.findByText("No User is Linked yet.")).resolves.toBeDefined();
  expect(hub.users).toStrictEqual([]);
});

test("an Account with no Server and no User says neither is Linked", async () => {
  openApp("/?settings=account-servers", accountHub().transport);

  const [servers, users] = [await group("Your Servers"), await group("Your Users")];

  await expect(servers.findByText("No Server is Linked yet.")).resolves.toBeDefined();
  await expect(users.findByText("No User is Linked yet.")).resolves.toBeDefined();
});
