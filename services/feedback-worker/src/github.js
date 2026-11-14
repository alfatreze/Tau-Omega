import { Refused } from './validate.js';

/** Files the issue. The token only ever lives in the Worker's secret and in this request header. */
export async function createIssue(env, issue) {
  let res;
  try {
    res = await fetch(`https://api.github.com/repos/${env.GITHUB_REPO}/issues`, {
      method: 'POST',
      headers: { Authorization: `Bearer ${env.GITHUB_TOKEN}`, Accept: 'application/vnd.github+json', 'X-GitHub-Api-Version': '2022-11-28', 'User-Agent': 'tau-omega-feedback', 'Content-Type': 'application/json' },
      body: JSON.stringify(issue),
    });
  } catch { throw new Refused(502, 'github_unreachable', 'GitHub could not be reached. Please try again in a moment.'); }
  if (res.status !== 201) {
    // Never pass GitHub's answer on: it can name the repository or the token's scope.
    throw new Refused(502, 'github_refused', 'The message could not be filed right now. Please try again later.');
  }
  const out = await res.json();
  return { number: out.number, url: out.html_url };
}
