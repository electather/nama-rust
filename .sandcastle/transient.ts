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

// GitHub also answers a good token with a one-off 401 now and then: on 2026-11-24 one `gh issue view` got "Bad
// credentials" between good calls, and the babysit loop stopped for the rest of the run. Only a gh command counts:
// a git auth failure (SSH, 1Password) or a Claude API 401 is a real infra failure. A token that's really revoked
// fails every call, so the caller bounds how often it believes this.

/** Whether `e` is a gh command GitHub refused with "Bad credentials". */
export function githubBadCredentials(e: unknown): boolean {
  const err = e as { message?: string; stderr?: unknown };
  const text = `${err?.message ?? e} ${err?.stderr ?? ""}`;
  return /Command failed: gh /.test(text) && /HTTP 401: Bad credentials/.test(text);
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

  assert.ok(githubBadCredentials(failed("gh issue view 2210 --json title,state,author,labels,parent",
    "HTTP 401: Bad credentials (https://api.github.com/graphql)\nTry authenticating with:  gh auth refresh -h github.com\n")));
  assert.ok(!githubBadCredentials(failed("git push origin agent/issue-1",
    "sign_and_send_pubkey: signing failed for ED25519 \"github\" from agent: agent refused operation\ngit@github.com: Permission denied (publickey).\n")),
    "git over SSH is infra");
  assert.ok(!githubBadCredentials(failed("git fetch origin main",
    "remote: Invalid username or token.\nfatal: Authentication failed for 'https://github.com/o/r.git/'\n")),
    "git over HTTPS is infra");
  assert.ok(!githubBadCredentials(new Error('API Error: 401 {"type":"error","error":{"type":"authentication_error","message":"Invalid bearer token"}}')),
    "a Claude API 401 is infra");
  assert.ok(!githubBadCredentials(failed("gh pr view 12", "HTTP 503: Service Unavailable (https://api.github.com/graphql)\n")));
}
