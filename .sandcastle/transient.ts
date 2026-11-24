// A git or gh call that GitHub answered with a 5xx. GitHub has these for seconds at a time, so only the issue that
// hit one crashes and the run goes on: one 500 on #1718's push stopped a whole run, then the supervisor waited an
// hour. The next run takes the issue again. Only a git or gh command counts, so a Docker or Claude API 500
// stays an infra failure.
// The message and stderr, not `instanceof`: execFileSync puts git's output in stderr.

/** Whether `e` is a git or gh command that failed on a GitHub server error. */
export function githubServerError(e: unknown): boolean {
  const err = e as { message?: string; stderr?: unknown };
  const text = `${err?.message ?? e} ${err?.stderr ?? ""}`;
  return /Command failed: (git|gh) /.test(text) &&
    /Internal Server Error|Bad Gateway|Service Unavailable|Gateway Time-?out|HTTP 50[0234]|returned error: 50[0234]/i.test(text);
}

// A loop's listing (open PRs, issues to pick or triage) runs every pass, so one failed gh call ended a whole run:
// a lone `HTTP 401: Bad credentials` while `gh auth status` was fine stopped babysitting and started nothing new,
// then the supervisor waited an hour. A failed pass is retried on the next; only LISTING_TRIES in a row, a
// failure that persists (auth really gone, network down), ends the run.

/** Consecutive failed listings after which a loop gives up and the run crashes. */
export const LISTING_TRIES = 5;

/** Counts a loop's consecutive failed listings: `fail` returns how many so far, `ok` starts the count again. */
export function failStreak() {
  let n = 0;
  return { fail: () => ++n, ok: () => { n = 0; } };
}

if (import.meta.main) {
  const { default: assert } = await import("node:assert/strict");
  const failed = (cmd: string, stderr: string) =>
    Object.assign(new Error(`Command failed: ${cmd}\n${stderr}`), { stderr });
  assert.ok(githubServerError(failed("git push --force-with-lease origin agent/issue-1718:agent/issue-1718",
    "remote: Internal Server Error\n ! [remote rejected] agent/issue-1718 -> agent/issue-1718 (Internal Server Error)\n")));
  assert.ok(githubServerError(failed("git fetch origin main",
    "fatal: unable to access 'https://github.com/o/r.git/': The requested URL returned error: 502\n")));
  assert.ok(githubServerError(failed("gh pr view 12", "HTTP 503: Service Unavailable (https://api.github.com/graphql)\n")));
  assert.ok(githubServerError(failed("gh api repos/o/r", "HTTP 504: 504 Gateway Time-out\n")));
  assert.ok(!githubServerError(failed("git push origin x", "remote: Invalid username or token.\nfatal: Authentication failed\n")),
    "git auth is infra");
  assert.ok(!githubServerError(failed("docker run sandcastle", "Error response from daemon: Internal Server Error\n")),
    "docker is infra");
  assert.ok(!githubServerError(new Error('API Error: 500 {"type":"api_error","message":"Internal server error"}')),
    "a Claude API error is infra");
  assert.ok(!githubServerError(failed("gh pr create", "HTTP 401: Bad credentials\n")));
  const streak = failStreak();
  assert.equal(streak.fail(), 1);
  assert.equal(streak.fail(), 2);
  streak.ok();
  assert.equal(streak.fail(), 1, "a good listing starts the count again");
}
