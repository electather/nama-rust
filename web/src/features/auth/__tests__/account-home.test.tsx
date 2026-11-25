import { create } from "@bufbuild/protobuf";
import { Code } from "@connectrpc/connect";
import { cleanup, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { UserInviteSchema } from "@/gen/nama/hub/v1/link_pb";
import { RefusalReason } from "@/gen/nama/hub/v1/refusal_pb";
import { openApp } from "@/test-app";
import { accountHub, hubRefused } from "@/test-hub";

afterEach(cleanup);

test("an Account with no Linked User is told to set up a Server or ask for an Invite", async () => {
  openApp("/", accountHub().transport);

  await expect(screen.findByText(/ask a Server’s Admin for an Invite/u)).resolves.toBeDefined();
});

const grace = create(UserInviteSchema, {
  id: "0197-grace",
  name: "Grace",
  serverName: "Cabin",
  invitedAt: 1_700_000_000n,
});

test("a User waiting for the Account shows instead of the notice, and accepting Links them", async () => {
  const { hub, transport } = accountHub({ invites: [grace] });
  openApp("/", transport);

  const accept = await screen.findByRole("button", { name: "Accept Grace on Cabin" });
  expect(screen.queryByText(/ask a Server’s Admin for an Invite/u)).toBeNull();
  fireEvent.click(accept);

  await waitFor(() => {
    expect(screen.queryByRole("heading", { name: "Users waiting for you" })).toBeNull();
  });
  expect(screen.queryByText(/ask a Server’s Admin for an Invite/u)).toBeNull();
  expect(hub.users.map((user) => user.name)).toStrictEqual(["Grace"]);
});

test("an Account whose email isn't proven is offered a Sign-in Link before accepting", async () => {
  const acceptRefusal = hubRefused(Code.FailedPrecondition, RefusalReason.EMAIL_NOT_PROVEN);
  openApp("/", accountHub({ invites: [grace], acceptRefusal }).transport);

  fireEvent.click(await screen.findByRole("button", { name: "Accept Grace on Cabin" }));

  await expect(screen.findByText(/Linking needs your email proven first/u)).resolves.toBeDefined();
  expect(screen.getByRole("link", { name: "Email me a Sign-in Link" })).toBeDefined();
});

test("linking a User by code takes the notice away", async () => {
  openApp("/", accountHub().transport);
  await screen.findByText(/ask a Server’s Admin for an Invite/u);

  const form = within(screen.getByRole("form", { name: "Link a User" }));
  fireEvent.change(form.getByLabelText("Code"), { target: { value: "USER-CODE" } });
  fireEvent.click(form.getByRole("button", { name: "Link User" }));

  await waitFor(() => {
    expect(screen.queryByText(/ask a Server’s Admin for an Invite/u)).toBeNull();
  });
});
