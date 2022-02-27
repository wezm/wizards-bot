// https://www.regextester.com/94502
const URL_REGEX =
  /(?:https?:\/\/)[\w.-]+(?:\.[\w\.-]+)+[\w\-\._~:/?#[\]@!\$&'\(\)\*\+,;=.]+/g;

export function substituteUrls(text: string): string {
  return text.replace(URL_REGEX, maybeReplaceUrl);
}

function maybeReplaceUrl(url0: string): string {
  const url = new URL(url0);

  if (url.host.endsWith("twitter.com")) {
    url.host = "nitter.net";
    return `${url} ([source](${url0}))`;
  } else if (url.host.endsWith("medium.com")) {
    url.host = "scribe.rip";
    return `${url} ([source](${url0}))`;
  } else {
    // Return untouched
    return url0;
  }
}
