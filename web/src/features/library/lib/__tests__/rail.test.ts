import { describe, expect, test } from "vite-plus/test";

import { type RailStop, railKeyName, railOf } from "@/features/library/lib/rail";

const at = (...keys: string[]) => keys.map((key, position) => ({ key, position }));

/** The rail's keys as it shows them, a dot for a gap. */
function shown(stops: RailStop[]) {
  return stops.map((stop) => (stop.kind === "key" ? stop.key : "·"));
}

describe("the quick-scroll rail", () => {
  test("puts one dot for each run of letters with no titles, in the listing's order", () => {
    const stops = shown(railOf(at("#", "A", "B", "D", "I", "O")));

    expect(stops).toStrictEqual(["#", "A", "B", "·", "D", "·", "I", "·", "O"]);
  });

  test("runs the other way for Z to A, and dots the years between those with titles", () => {
    const letters = shown(railOf(at("O", "N", "D")));
    const years = shown(railOf(at("2024", "2023", "2017", "2016", "2014")));

    expect(letters).toStrictEqual(["O", "N", "·", "D"]);
    expect(years).toStrictEqual(["2024", "2023", "·", "2017", "2016", "·", "2014"]);
  });

  test("names each key by what it jumps to", () => {
    expect(railKeyName("A", "Movies")).toBe("Movies starting with A");
    expect(railKeyName("#", "Shows")).toBe("Shows starting with a number or symbol");
    expect(railKeyName("2014", "Movies")).toBe("Movies from 2014");
  });
});
