/** A run of text in a Plugin's setup steps: plain, bold, code, or a link to a web page. */
export type SetupRun =
  | { kind: "text" | "strong" | "code"; text: string }
  | { kind: "link"; text: string; href: string };

/** A paragraph, bulleted list or numbered list of a Plugin's setup steps. */
export type SetupBlock =
  | { kind: "paragraph"; runs: SetupRun[] }
  | { kind: "bullets" | "steps"; items: SetupRun[][] };

const bullet = /^\s*[-*+]\s+(?<rest>.*)$/u;
const step = /^\s*\d+[.)]\s+(?<rest>.*)$/u;
const inlineRun =
  /\*\*(?<strong>.+?)\*\*|`(?<code>[^`]+)`|\[(?<label>[^\]]+)\]\((?<href>(?:[^\s()]|\([^\s()]*\))+)\)/gu;

/**
 * `href` as an absolute http or https URL, or `undefined` for any other scheme or none: the only
 * links a Plugin's page renders (`src/features/admin-plugins/AGENTS.md`).
 */
export function webUrl(href: string): string | undefined {
  const url = URL.parse(href);
  return url?.protocol === "http:" || url?.protocol === "https:" ? url.href : undefined;
}

/**
 * A Plugin's setup steps, read as the small Markdown subset its page shows: paragraphs, lists,
 * bold, inline code and links. Anything else, raw HTML included, stays text, since the page puts
 * it in the DOM only as text (ADR 0058, `src/features/admin-plugins/AGENTS.md`).
 */
export function parseSetupMarkdown(source: string): SetupBlock[] {
  const blocks: LineBlock[] = [];
  let afterBlank = false;
  // ponytail: no nested lists, `start` numbers, or links or code in bold; add them if a Plugin needs them.
  for (const line of source.split(/\r?\n/u)) {
    const blank = line.trim() === "";
    if (!blank) {
      addLine(blocks, line, afterBlank);
    }
    afterBlank = blank;
  }
  return blocks.map(({ kind, items }) =>
    kind === "paragraph"
      ? { kind, runs: runsOf(items.join(" ")) }
      : { kind, items: items.map((text) => runsOf(text)) },
  );
}

/** A `SetupBlock` whose items are still their Markdown. */
interface LineBlock {
  kind: SetupBlock["kind"];
  items: string[];
}

/**
 * Adds `line` to the end of `blocks`. A list item joins the list before it even across a blank
 * line, so a loose numbered list keeps counting.
 */
function addLine(blocks: LineBlock[], line: string, afterBlank: boolean) {
  const item = listItem(line);
  const last = blocks.at(-1);
  if (item !== undefined && last?.kind === item.kind) {
    last.items.push(item.text);
  } else if (item !== undefined) {
    blocks.push({ kind: item.kind, items: [item.text] });
  } else if (last === undefined || afterBlank) {
    blocks.push({ kind: "paragraph", items: [line.trim()] });
  } else {
    // A line under a paragraph or list item continues it, as CommonMark's lazy continuation does.
    last.items.push(`${last.items.pop() ?? ""} ${line.trim()}`);
  }
}

/** `line` as a bulleted or numbered list item, or `undefined` if it isn't one. */
function listItem(line: string) {
  const bulleted = bullet.exec(line)?.groups?.["rest"];
  if (bulleted !== undefined) {
    return { kind: "bullets" as const, text: bulleted };
  }
  const numbered = step.exec(line)?.groups?.["rest"];
  return numbered === undefined ? undefined : { kind: "steps" as const, text: numbered };
}

function runsOf(text: string): SetupRun[] {
  const runs: SetupRun[] = [];
  let at = 0;
  for (const match of text.matchAll(inlineRun)) {
    if (match.index > at) {
      runs.push({ kind: "text", text: text.slice(at, match.index) });
    }
    runs.push(runOfMatch(match.groups ?? {}));
    at = match.index + match[0].length;
  }
  if (at < text.length) {
    runs.push({ kind: "text", text: text.slice(at) });
  }
  return runs;
}

/** The run one match of `inlineRun` stands for. A link to anything but a web page stays its label. */
function runOfMatch({
  strong,
  code,
  label = "",
  href = "",
}: Record<string, string | undefined>): SetupRun {
  if (strong !== undefined) {
    return { kind: "strong", text: strong };
  }
  if (code !== undefined) {
    return { kind: "code", text: code };
  }
  const url = webUrl(href);
  return url === undefined
    ? { kind: "text", text: label }
    : { kind: "link", text: label, href: url };
}
