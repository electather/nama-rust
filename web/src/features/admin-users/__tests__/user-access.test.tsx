import type { MessageInitShape } from "@bufbuild/protobuf";
import { cleanup, fireEvent, screen } from "@testing-library/react";
import { afterEach, describe, expect, test } from "vite-plus/test";

import type { LibraryAccessSchema, MaximumAgeSchema } from "@/gen/nama/v1/sign_in_pb";
import { admin, fakeServer, newUser, openApp } from "@/test-app";
import { dropdown, pick } from "@/test-settings";

afterEach(cleanup);

/** Settings' Users section, the address a test opens it at. */
const usersSection = "/code?settings=users";

/** Opens `name`'s details from the list of Users, once they're in its place. */
async function openUser(name: string) {
  fireEvent.click(await screen.findByRole("link", { name }));
  await screen.findByRole("region", { name });
}

/** A fake Server keeping Alex's Maximum Age and Libraries as they're set, with two Libraries. */
function keepingAlex() {
  const alex: {
    maximumAge?: MessageInitShape<typeof MaximumAgeSchema> | undefined;
    libraryAccess?: MessageInitShape<typeof LibraryAccessSchema> | undefined;
  } = {};
  const server = fakeServer(
    {
      signedIn: admin,
      libraries: [
        { id: 1n, name: "Movies" },
        { id: 2n, name: "Cartoons" },
      ],
    },
    {},
    {
      listUsers: () => ({ users: [{ id: 2n, name: "Alex", permissions: newUser, ...alex }] }),
      setMaximumAge: ({ maximumAge }) => {
        alex.maximumAge = maximumAge;
        return {};
      },
      setLibraryAccess: ({ libraryAccess }) => {
        alex.libraryAccess = libraryAccess;
        return {};
      },
    },
  );
  return { alex, server };
}

/** Opens the app afresh on Alex's details. */
async function reopenAlex(server: ReturnType<typeof fakeServer>) {
  cleanup();
  openApp(usersSection, server);
  await openUser("Alex");
}

describe("an Admin setting a User's Maximum Age", () => {
  test("is saved as it's picked, and is there after reloading", async () => {
    const { alex, server } = keepingAlex();
    await reopenAlex(server);

    expect(dropdown("Maximum Age").textContent).toBe("None");
    expect(
      screen.getByRole("switch", { name: "Allow unrated titles" }).getAttribute("aria-disabled"),
    ).toBe("true");
    pick("Maximum Age", "13");
    await expect.poll(() => alex.maximumAge?.age).toBe(13);
    fireEvent.click(screen.getByRole("switch", { name: "Allow unrated titles" }));
    await expect.poll(() => alex.maximumAge?.allowUnrated).toBe(true);
    await reopenAlex(server);

    expect(dropdown("Maximum Age").textContent).toBe("13");
    expect(
      screen.getByRole("switch", { name: "Allow unrated titles" }).getAttribute("aria-checked"),
    ).toBe("true");
  });

  test("is cleared by picking None", async () => {
    const { alex, server } = keepingAlex();
    alex.maximumAge = { age: 16, allowUnrated: false };
    await reopenAlex(server);

    pick("Maximum Age", "None");

    await expect.poll(() => alex.maximumAge).toBeUndefined();
  });
});

describe("an Admin setting a User's Libraries", () => {
  test("chooses which they see, starting from every Library, and it's there after reloading", async () => {
    const { alex, server } = keepingAlex();
    await reopenAlex(server);

    expect(dropdown("Libraries").textContent).toBe("All Libraries");
    expect(screen.queryByRole("switch", { name: "Movies" })).toBeNull();
    pick("Libraries", "Chosen Libraries");
    await expect.poll(() => alex.libraryAccess?.libraryIds).toStrictEqual([1n, 2n]);
    fireEvent.click(screen.getByRole("switch", { name: "Movies" }));
    await expect.poll(() => alex.libraryAccess?.libraryIds).toStrictEqual([2n]);
    await reopenAlex(server);

    expect(dropdown("Libraries").textContent).toBe("Chosen Libraries");
    expect(screen.getByRole("switch", { name: "Movies" }).getAttribute("aria-checked")).toBe(
      "false",
    );
    expect(screen.getByRole("switch", { name: "Cartoons" }).getAttribute("aria-checked")).toBe(
      "true",
    );
  });

  test("goes back to every Library by picking All Libraries", async () => {
    const { alex, server } = keepingAlex();
    alex.libraryAccess = { libraryIds: [2n] };
    await reopenAlex(server);

    pick("Libraries", "All Libraries");

    await expect.poll(() => alex.libraryAccess).toBeUndefined();
  });
});
