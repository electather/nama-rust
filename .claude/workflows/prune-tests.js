export const meta = {
  name: 'prune-tests',
  description: 'Haiku triages a package\'s tests for unnecessary ones; Sonnet verifies each and removes the confirmed',
  whenToUse: 'args: "crates/nama-server" or {target, root?, batch?, dryRun?}. Edits the working tree.',
  phases: [
    { title: 'Discover', detail: 'list the test files under the target', model: 'haiku' },
    { title: 'Triage', detail: 'Haiku flags tests that may be unnecessary', model: 'haiku' },
    { title: 'Verify', detail: 'Sonnet confirms or rejects each flag, removes the confirmed', model: 'sonnet' },
    { title: 'Cleanup', detail: 'fix dead code the removals left behind', model: 'sonnet' },
  ],
}

const opts = typeof args === 'string' ? { target: args } : (args || {})
const target = opts.target
if (!target) throw new Error('pass a target, e.g. args: "crates/nama-server"')
const BATCH = opts.batch || 3
const dryRun = !!opts.dryRun
// Agents start in the session's checkout; `root` points them at another (e.g. a clean worktree).
const AT = opts.root ? `Work only in the checkout at \`${opts.root}\`: cd there for every command; paths below are relative to it.\n\n` : ''

const FILES = {
  type: 'object',
  properties: { files: { type: 'array', items: { type: 'string' } } },
  required: ['files'],
}
const CANDIDATES = {
  type: 'object',
  properties: {
    candidates: {
      type: 'array',
      items: {
        type: 'object',
        properties: {
          file: { type: 'string' },
          test: { type: 'string', description: 'test function name' },
          reason: { type: 'string', enum: ['duplicate', 'covered-by-broader', 'tests-dependency-or-language', 'tautological', 'tests-a-mock', 'dead-behaviour', 'other'] },
          evidence: { type: 'string', description: 'the covering test (file::name) or why it proves nothing' },
        },
        required: ['file', 'test', 'reason', 'evidence'],
      },
    },
  },
  required: ['candidates'],
}
const VERDICTS = {
  type: 'object',
  properties: {
    verdicts: {
      type: 'array',
      items: {
        type: 'object',
        properties: {
          test: { type: 'string' },
          remove: { type: 'boolean' },
          removed: { type: 'boolean', description: 'true only if you actually deleted it' },
          why: { type: 'string' },
        },
        required: ['test', 'remove', 'removed', 'why'],
      },
    },
  },
  required: ['verdicts'],
}

phase('Discover')
const { files } = await agent(
  AT + `List every file under \`${target}\` that contains tests: Rust files with #[test], #[tokio::test] or another test attribute, and TS/JS *.test.* / *.spec.* files. Use grep -rlE, skip target/ and node_modules/. Return repo-relative paths.`,
  { label: 'discover', model: 'haiku', effort: 'low', schema: FILES },
)
const batches = []
for (let i = 0; i < files.length; i += BATCH) batches.push(files.slice(i, i + BATCH))
log(`${files.length} test files in ${batches.length} batches${dryRun ? ' (dry run)' : ''}`)

const triage = batch => agent(
  AT + `You triage tests for removal. Read every test in these files:
${batch.map(f => `- ${f}`).join('\n')}

Flag a test only when one of these holds, and give concrete evidence:
- duplicate: another test (name it as file::name) checks the same behaviour with the same kind of input.
- covered-by-broader: a broader test (name it) already fails whenever this one would.
- tests-dependency-or-language: it checks std, serde, a derive or a library rather than this code.
- tautological: it asserts what it just set up, or asserts nothing that could fail.
- tests-a-mock: it only exercises a fake, never the real code.
- dead-behaviour: the code it tests is unused or gone.
Never flag a test that enforces an AGENTS.md rule or ADR (e.g. tests/structure.rs), a regression test citing an issue (#123), or the only test of a behaviour. Grep the package to find covering tests. When unsure, don't flag. Do not edit anything.`,
  { label: `triage:${batch[0].split('/').pop()}`, phase: 'Triage', model: 'haiku', effort: 'medium', schema: CANDIDATES },
)

// One verifier per file, so parallel edits never touch the same file.
const verify = (file, cands) => agent(
  AT + `Haiku flagged these tests in \`${file}\` as unnecessary. It is often wrong; your job is to check each claim independently.
${cands.map(c => `- ${c.test} [${c.reason}]: ${c.evidence}`).join('\n')}

For each: read the test and the code it exercises. If it claims a covering test, open that test and confirm it fails in every case this one would (same paths, same edge cases, same assertions). Keep the test (remove=false) if it covers any branch, input, error path or regression the others don't, enforces an AGENTS.md rule or ADR, or if you are unsure.
${dryRun
    ? 'DRY RUN: do not edit any file; set removed=false.'
    : `For each test with remove=true, delete it from ${file}, plus imports and file-local helpers only it used. If the file is left with no tests, delete the file and its \`#[cfg(test)] mod\` declaration. Don't touch other tests or shared fixtures.`}`,
  { label: `verify:${file.split('/').pop()}`, phase: 'Verify', model: 'sonnet', effort: 'high', schema: VERDICTS },
).then(r => r && r.verdicts.map(v => ({ file, ...v })))

const results = await pipeline(
  batches,
  triage,
  r => {
    const byFile = {}
    for (const c of (r ? r.candidates : [])) (byFile[c.file] ||= []).push(c)
    return parallel(Object.entries(byFile).map(([f, cs]) => () => verify(f, cs)))
  },
)
const verdicts = results.filter(Boolean).flat().filter(Boolean).flat()
const removed = verdicts.filter(v => v.removed)
const kept = verdicts.filter(v => !v.remove)
log(`${verdicts.length} flagged, ${removed.length} removed, ${kept.length} kept`)

let cleanup = null
if (removed.length) {
  phase('Cleanup')
  cleanup = await agent(
    AT + `Tests were just removed from these files under \`${target}\`:
${[...new Set(removed.map(v => v.file))].map(f => `- ${f}`).join('\n')}
Shared test helpers or fixtures may now be dead. Run the package's lint over its tests (for a Rust crate: cargo clippy -p <crate> --all-targets --all-features -- -D warnings; for web/: its lint and typecheck), redirecting output to a file in your scratchpad and reading only the failures. Fix only dead code and unused imports the removals caused; restore nothing. Return a short list of what you changed and anything still failing.`,
    { label: 'cleanup', model: 'sonnet', effort: 'high' },
  )
}

return { target, dryRun, removed, kept, cleanup }
