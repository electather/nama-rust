// Order matters: Edge and Opera say "Chrome" too, and Chrome says "Safari".
const browsers: [mark: string, name: string][] = [
  ["Edg/", "Edge"],
  ["OPR/", "Opera"],
  ["Firefox/", "Firefox"],
  ["Chrome/", "Chrome"],
  ["Safari/", "Safari"],
];
// iPhones and iPads say "Mac OS X" too, and Android says "Linux".
const systems: [mark: string, name: string][] = [
  ["iPhone", "iPhone"],
  ["iPad", "iPad"],
  ["Android", "Android"],
  ["CrOS", "ChromeOS"],
  ["Mac OS X", "Mac"],
  ["Windows", "Windows"],
  ["Linux", "Linux"],
];

/**
 * A name for this browser that its User would recognise, such as "Chrome on Mac", from its
 * `userAgent`, or `""` when it's one we don't know.
 */
export function browserName(userAgent: string) {
  const browser = browsers.find(([mark]) => userAgent.includes(mark))?.[1];
  const system = systems.find(([mark]) => userAgent.includes(mark))?.[1];
  if (browser === undefined) {
    return system ?? "";
  }
  return system === undefined ? browser : `${browser} on ${system}`;
}
