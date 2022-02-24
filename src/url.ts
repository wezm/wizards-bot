// https://www.regextester.com/94502
const URL_REGEX = /(?:https?:\/\/)[\w.-]+(?:\.[\w\.-]+)+[\w\-\._~:/?#[\]@!\$&'\(\)\*\+,;=.]+/g;

export function twitterToNitter(text: string): string {
    return text.replace(URL_REGEX, (url) => twitterUrlToNitterUrl(url).toString());
}

function twitterUrlToNitterUrl(url0: string): URL {
  const url = new URL(url0);
  if (url.host === "twitter.com" || url.host === "mobile.twitter.com") {
    url.host = "nitter.net";
  }

  return url;
}

