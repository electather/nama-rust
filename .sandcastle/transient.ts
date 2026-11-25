// A git or gh call that GitHub answered with a 5xx, or that never reached it. GitHub has these for seconds at a
// time, so only the issue that hit one crashes and the run goes on: one 500 on #1718's push stopped a whole run,
// then the supervisor waited an hour. The next run takes the issue again. A dial timeout listing PRs (2026-11-25)
// and a push GitHub failed to store (`remote unpack failed`, 2026-11-24) ended runs the same way. Only a git or gh
// command counts, so a Docker or Claude API 500 stays an infra failure.
// The message and stderr, not `instanceof`: execFileSync puts git's output in stderr.

const SERVER_ERROR = /Internal Server Error|Bad Gateway|Service Unavailable|Gateway Time-?out|HTTP 50[0234]|returned error: 50[0234]/i;
const NETWORK_ERROR = new RegExp([
  "i/o timeout", "dial tcp", "connection reset by peer", "connection refused", "TLS handshake timeout",
  "unexpected EOF", "Could not resolve host", "Failed to connect to github\\.com", "RPC failed", "early EOF",
  "remote unpack failed", "index-pack failed", "the remote end hung up unexpectedly",
].join("|"), "i");

/** Whether `e` is a git or gh command that failed on a GitHub server error or a network failure on the way there. */
export function githubUnavailable(e: unknown): boolean {
  const err = e as { message?: string; stderr?: unknown };
  const text = `${err?.message ?? e} ${err?.stderr ?? ""}`;
  return /Command failed: (git|gh) /.test(text) && (SERVER_ERROR.test(text) || NETWORK_ERROR.test(text));
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
  assert.ok(githubUnavailable(failed("git push --force-with-lease origin agent/issue-1718:agent/issue-1718",
    "remote: Internal Server Error\n ! [remote rejected] agent/issue-1718 -> agent/issue-1718 (Internal Server Error)\n")));
  assert.ok(githubUnavailable(failed("git fetch origin main",
    "fatal: unable to access 'https://github.com/o/r.git/': The requested URL returned error: 502\n")));
  assert.ok(githubUnavailable(failed("gh pr view 12", "HTTP 503: Service Unavailable (https://api.github.com/graphql)\n")));
  assert.ok(githubUnavailable(failed("gh api repos/o/r", "HTTP 504: 504 Gateway Time-out\n")));
  assert.ok(!githubUnavailable(failed("git push origin x", "remote: Invalid username or token.\nfatal: Authentication failed\n")),
    "git auth is infra");
  assert.ok(!githubUnavailable(failed("docker run sandcastle", "Error response from daemon: Internal Server Error\n")),
    "docker is infra");
  assert.ok(!githubUnavailable(new Error('API Error: 500 {"type":"api_error","message":"Internal server error"}')),
    "a Claude API error is infra");
  assert.ok(!githubUnavailable(failed("gh pr create", "HTTP 401: Bad credentials\n")));
  assert.ok(githubUnavailable(failed(
    "gh pr list --state open --limit 100 --json number,isDraft,isCrossRepository,headRefName,author,closingIssuesReferences",
    'Post "https://api.github.com/graphql": dial tcp 20.26.156.210:443: i/o timeout\n')));
  assert.ok(githubUnavailable(failed("git push --force-with-lease origin agent/issue-2193:agent/issue-2193",
    "remote: error: unable to write file /data/repositories/o/r.git/objects/pack/pack-1.pack: No such file or directory\n" +
      "error: remote unpack failed: index-pack failed\nTo https://github.com/o/r.git\n" +
      " ! [remote rejected]     agent/issue-2193 -> agent/issue-2193 (failed)\n")));
  assert.ok(!githubUnavailable(failed("docker run sandcastle",
    "docker: Error response from daemon: dial tcp 10.0.0.1:443: i/o timeout\n")), "docker's network is infra");
  assert.ok(!githubUnavailable(failed("git push origin x",
    "error: failed to push some refs\n ! [rejected] x -> x (non-fast-forward)\n")), "a rejected push is the issue's own");

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
