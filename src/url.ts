// https://www.regextester.com/94502
const URL_REGEX = /(?:https?:\/\/)[\w.-]+(?:\.[\w\.-]+)+[\w\-\._~:/?#[\]@!\$&'\(\)\*\+,;=.]+/g;

export function twitterToNitter(text: string): string {
    return substituteUrls(text, twitterUrlToNitterUrl);
}

export function mediumToScribe(text: string): string {
    return substituteUrls(text, mediumUrlToScribeUrl);
}

function substituteUrls(text: string, fn: (url: string) => URL): string {
    return text.replace(URL_REGEX, url => fn(url).toString());
}

function twitterUrlToNitterUrl(url0: string): URL {
  const url = new URL(url0);
  if (url.host.endsWith("twitter.com")) {
    url.host = "nitter.net";
  }

  return url;
}

function mediumUrlToScribeUrl(url0: string): URL {
  const url = new URL(url0);
  if (url.host.endsWith("medium.com")) {
    url.host = "scribe.rip";
  }

  return url;
}
