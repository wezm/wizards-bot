mod bushfire;
mod datastore;

use std::borrow::Cow;
use std::error::Error;
use std::net::ToSocketAddrs;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use std::{env, io, process, thread};

use json::{object, JsonValue};
use once_cell::sync::Lazy;
use regex::{Captures, Regex};
use time::format_description::well_known::Rfc2822;
use tiny_http::{Header, HeaderField, Method, Request, Response, StatusCode};
use url::Url;

use crate::bushfire::Entry;

const HTML: &str = include_str!("home.html");
const CSS: &str = include_str!("style.css");
const NOT_FOUND: &str = include_str!("not_found.html");
const ONE_SECOND: Duration = Duration::from_secs(1);
/// Poll the bushfire feed every 5 minutes
const POLL_BUSHFIRE_FEED: u32 = 5 * 60;
const BUSHFIRE_PAGE: &str = "https://www.qfes.qld.gov.au/Current-Incidents";

// NOTE(unwrap): These are known valid
static AUTHORIZATION: Lazy<HeaderField> = Lazy::new(|| "Authorization".parse().unwrap());
static CONTENT_TYPE: Lazy<HeaderField> = Lazy::new(|| "Content-Type".parse().unwrap());
static JSON_CONTENT_TYPE: Lazy<Header> = Lazy::new(|| {
    "Content-type: application/json; charset=utf-8"
        .parse()
        .unwrap()
});
static HTML_CONTENT_TYPE: Lazy<Header> =
    Lazy::new(|| "Content-type: text/html; charset=utf-8".parse().unwrap());
static CSS_CONTENT_TYPE: Lazy<Header> =
    Lazy::new(|| "Content-type: text/css; charset=utf-8".parse().unwrap());
static HOME_HTML: Lazy<String> = Lazy::new(|| {
    let git_rev = env::var("WIZARDS_BOT_REVISION").unwrap_or_else(|_| String::from("dev"));
    HTML.replace("$rev$", &git_rev)
});

fn main() -> Result<(), io::Error> {
    let term = Arc::new(AtomicBool::new(false));
    signal_hook::flag::register(signal_hook::consts::SIGINT, Arc::clone(&term))?;
    signal_hook::flag::register(signal_hook::consts::SIGTERM, Arc::clone(&term))?;

    let mut threads = Vec::new();

    let mm_token = env::var_os("MM_SLASH_TOKEN");
    let mm_token = mm_token
        .as_ref()
        .ok_or_else(|| io::Error::new(io::ErrorKind::Other, "MM_SLASH_TOKEN is not set"))
        .and_then(|token| {
            token.to_str().ok_or_else(|| {
                io::Error::new(io::ErrorKind::Other, "MM_SLASH_TOKEN is not valid UTF-8")
            })
        })?;
    let mm_webhook = env::var_os("MM_BUSHFIRE_WEBHOOK");
    let mm_webhook = mm_webhook
        .as_ref()
        .ok_or_else(|| io::Error::new(io::ErrorKind::Other, "MM_BUSHFIRE_WEBHOOK is not set"))
        .and_then(|webhook| {
            webhook.to_str().ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::Other,
                    "MM_BUSHFIRE_WEBHOOK is not valid UTF-8",
                )
            })
        })?;

    let data_path = env::var_os("WIZARDS_BOT_DATA_PATH");
    let data_path = data_path
        .as_ref()
        .map(Path::new)
        .ok_or_else(|| io::Error::new(io::ErrorKind::Other, "WIZARDS_BOT_DATA_PATH is not set"))?;

    let bushfire_point = env::var_os("WIZARDS_BOT_BUSHFIRE_POINT");
    let bushfire_point = bushfire_point
        .as_ref()
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::Other,
                "WIZARDS_BOT_BUSHFIRE_POINT is not set",
            )
        })
        .and_then(|webhook| {
            webhook
                .to_str()
                .and_then(|s| s.split_once(','))
                .and_then(|(lat, long)| match (lat.parse(), long.parse()) {
                    (Ok(lat), Ok(long)) => Some((lat, long)),
                    _ => None,
                })
                .ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::Other,
                        "Unable to parse WIZARDS_BOT_BUSHFIRE_POINT",
                    )
                })
        })?;
    println!(
        "INFO: monitoring for bushfire events at {}, {}",
        bushfire_point.0, bushfire_point.1
    );

    let datastore = datastore::Datastore::new(data_path)
        .map(|store| Arc::new(Mutex::new(store)))
        .map_err(|err| {
            io::Error::new(
                io::ErrorKind::Other,
                format!("unable to open datastore at {}: {err}", data_path.display()),
            )
        })?;

    let server_addr = (
        env::var("WIZARDS_BOT_ADDRESS").unwrap_or_else(|_| String::from("0.0.0.0")),
        env::var("WIZARDS_BOT_PORT")
            .ok()
            .and_then(|port| port.parse::<u16>().ok())
            .unwrap_or(8888),
    );
    let server = match Server::new(server_addr.clone(), mm_token) {
        Ok(server) => Arc::new(server),
        Err(err) => {
            eprintln!(
                "ERROR: Unable to start http server on {}:{}: {}",
                server_addr.0, server_addr.1, err
            );
            process::exit(1);
        }
    };
    println!(
        "INFO: http server running on http://{}:{}",
        server_addr.0, server_addr.1
    );

    // Handle HTTP requests
    {
        let server = Arc::clone(&server);
        let thread = thread::spawn(move || {
            server.handle_requests();
            println!("INFO: server thread exiting");
        });
        threads.push(thread);
    }

    // Set to the trigger value to cause an initial check on startup
    let mut bushfire_wait = POLL_BUSHFIRE_FEED;

    // Wait for signals to exit
    while !term.load(Ordering::Relaxed) {
        thread::sleep(ONE_SECOND);
        bushfire_wait += 1;
        if bushfire_wait >= POLL_BUSHFIRE_FEED {
            bushfire_wait = 0;
            let entries = match bushfire::check(bushfire_point) {
                Ok(entries) => {
                    println!("INFO: polled bushfire feed");
                    entries
                }
                Err(err) => {
                    let _ =
                        post_webhook(&format!("unable to poll bushfire feed: {err}"), mm_webhook);
                    continue;
                }
            };
            if !entries.is_empty() {
                let mut datastore = datastore.lock().unwrap();
                for entry in entries {
                    if !datastore.contains(&entry.id) {
                        // notify about this entry
                        println!("INFO: notify of incident {}", entry.id.0);
                        match notify_entry(&entry, mm_webhook) {
                            Ok(()) => {
                                match datastore.append(entry.id) {
                                    Ok(()) => (),
                                    Err(err) => {
                                        if let Err(notify_err) = post_webhook(
                                            &format!("Unable to append entry to bushfire datastore: {err}"),
                                            mm_webhook,
                                        ) {
                                            eprintln!("ERROR: Unable to append entry to bushfire datastore: {err}, error posting notification about that error: {notify_err}")
                                        }
                                        continue;
                                    }
                                }
                            }
                            Err(err) => {
                                eprintln!(
                                    "ERROR: Unable to post notification: {}: {}",
                                    err.error, err.notification
                                )
                            }
                        }
                    }
                }
            }
        }
    }
    server.shutdown();

    for thread in threads {
        let _ = thread.join();
    }

    Ok(())
}

pub struct Server {
    server: tiny_http::Server,
    mattermost_token: String,
}

impl Server {
    pub fn new<A>(addr: A, token: &str) -> Result<Server, Box<dyn Error + Send + Sync + 'static>>
    where
        A: ToSocketAddrs,
    {
        let mattermost_token = format!("Token {}", token);
        tiny_http::Server::http(addr).map(|server| Server {
            server,
            mattermost_token,
        })
    }

    pub fn handle_requests(&self) {
        for mut request in self.server.incoming_requests() {
            let response = match request.url() {
                "/" => Response::from_string(&*HOME_HTML).with_header(HTML_CONTENT_TYPE.clone()),
                "/nit" => {
                    if request.method() == &Method::Post {
                        let (obj, status) = self.nit_slash_command(&mut request);
                        let body = json::stringify_pretty(obj, 2);
                        Response::from_string(body)
                            .with_header(JSON_CONTENT_TYPE.clone())
                            .with_status_code(status)
                    } else {
                        Response::from_string(NOT_FOUND)
                            .with_header(HTML_CONTENT_TYPE.clone())
                            .with_status_code(404)
                    }
                }
                "/style.css" => Response::from_string(CSS).with_header(CSS_CONTENT_TYPE.clone()),
                _ => Response::from_string(NOT_FOUND)
                    .with_header(HTML_CONTENT_TYPE.clone())
                    .with_status_code(404),
            };

            // Ignoring I/O errors that occur here so that we don't take down the process if there
            // is an issue sending the response.
            let _ = request.respond(response);
        }
    }

    fn nit_slash_command(&self, request: &mut Request) -> (JsonValue, StatusCode) {
        let (content_type, authorization) = match Self::validate_request(request) {
            Ok(headers) => headers,
            Err((message, status)) => {
                return (object! {error: message}, status);
            }
        };

        if content_type.value != "application/x-www-form-urlencoded" {
            return (object! {error: "Bad request"}, StatusCode::from(400));
        }

        if !self.verify_token(authorization.value.as_str()) {
            return (object! {error: "Not authorised"}, StatusCode::from(401));
        }

        // Get the text field of the form data
        let mut body = Vec::new();
        if request.as_reader().read_to_end(&mut body).is_err() {
            return (
                object! {error: "Internal server error"},
                StatusCode::from(500),
            );
        }
        match form_urlencoded::parse(&body).find(|(key, _value)| key == "text") {
            Some((_key, text)) if !is_blank(&text) => (
                object! {
                  "response_type": "in_channel",
                  "text": &*substitute_urls(&text),
                },
                StatusCode::from(200),
            ),
            Some(_) | None => (
                object! {
                    "response_type": "ephemeral",
                    "text": "You need to supply some text",
                },
                StatusCode::from(200),
            ),
        }
    }

    fn validate_request(request: &Request) -> Result<(&Header, &Header), (String, StatusCode)> {
        const BAD_REQUEST: u16 = 400;

        // Extract required headers
        let content_type = request
            .headers()
            .iter()
            .find(|&header| header.field == *CONTENT_TYPE)
            .ok_or_else(|| {
                (
                    String::from("Content-Type header not found"),
                    StatusCode::from(BAD_REQUEST),
                )
            })?;
        let authorization = request
            .headers()
            .iter()
            .find(|&header| header.field == *AUTHORIZATION)
            .ok_or_else(|| {
                (
                    String::from("Authorization header not found"),
                    StatusCode::from(BAD_REQUEST),
                )
            })?;
        Ok((content_type, authorization))
    }

    fn verify_token(&self, token: &str) -> bool {
        token == self.mattermost_token
    }

    pub fn shutdown(&self) {
        self.server.unblock();
    }
}

struct NotifyError {
    notification: String,
    error: ureq::Error,
}

fn notify_entry(entry: &Entry, webhook: &str) -> Result<(), NotifyError> {
    let message = format!(
        "#### ⚠️ {}\n\n**{}**\n\n{}\n\n**Published:** {}\n**Link:** {}",
        entry.category.as_deref().unwrap_or("Unknown Category"),
        entry.title.as_deref().unwrap_or("Untitled"),
        entry.content.as_deref().unwrap_or("No content"),
        entry
            .published
            .and_then(|published| published.format(&Rfc2822).ok())
            .as_deref()
            .unwrap_or("unknown"),
        BUSHFIRE_PAGE
    );
    post_webhook(&message, webhook).map_err(|error| NotifyError {
        notification: message,
        error,
    })
}

fn post_webhook(message: &str, webhook: &str) -> Result<(), ureq::Error> {
    let body = object! {
        text: message
    };

    ureq::post(webhook)
        .set("Content-Type", "application/json")
        .send_string(&json::stringify(body))
        .map(drop)
}

fn is_blank(text: &str) -> bool {
    text.chars().all(|ch| ch.is_whitespace())
}

static URL_REGEX: Lazy<Regex> = Lazy::new(||
    // https://www.regextester.com/94502
    Regex::new(r"https?://[[:word:].-]+(?:\.[[:word:].-]+)+[[:word:]\-._~:/?#\[\]@!$&'()*+,;=]+").unwrap());

fn substitute_urls(text: &str) -> Cow<'_, str> {
    URL_REGEX.replace_all(text, maybe_replace_url)
}

fn maybe_replace_url(captures: &Captures<'_>) -> String {
    // NOTE(unwrap): captures 0 should always be present and it should be parseable as a URL due
    // to matching the regex.
    let url0 = captures.get(0).unwrap().as_str();
    let mut url: Url = url0.parse().unwrap();

    if url.host_str().map_or(false, |host| {
        host == "x.com" || host.ends_with("twitter.com")
    }) {
        let _ = url.set_host(Some("nitter.net"));
        // Nitter doesn't like Twitter's new tracking params so strip query string and hope for the
        // best.
        url.set_query(None);
        format!("{} ([source]({}))", url, url0)
    } else if url
        .host_str()
        .map_or(false, |host| host.ends_with("medium.com"))
    {
        let _ = url.set_host(Some("scribe.rip"));
        format!("{} ([source]({}))", url, url0)
    } else {
        // Return original url
        url0.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn twitter_to_nitter_desktop() {
        let val = substitute_urls("https://twitter.com/wezm");
        assert_eq!(
            val,
            "https://nitter.net/wezm ([source](https://twitter.com/wezm))",
        );
    }

    #[test]
    fn x_to_nitter_desktop() {
        let val = substitute_urls("https://x.com/wezm");
        assert_eq!(
            val,
            "https://nitter.net/wezm ([source](https://x.com/wezm))",
        );
    }

    #[test]
    fn twitter_to_nitter_mobile() {
        let val = substitute_urls(
        "https://mobile.twitter.com/wezm/status/1323096439602339840?s=20&t=Zper7b85RVlpWoTKKJDkbg",
    );
        assert_eq!(
            val,
            "https://nitter.net/wezm/status/1323096439602339840 ([source](https://mobile.twitter.com/wezm/status/1323096439602339840?s=20&t=Zper7b85RVlpWoTKKJDkbg))",
        );
    }

    #[test]
    fn twitter_to_nitter_multiple() {
        let val = substitute_urls(
            "Here is some things from twitter.com https://twitter.com/wezm/status/1323096439602339840?s=20&t=Zper7b85RVlpWoTKKJDkbg and https://twitter.com/rustlang/status/1496894318887546883?s=20&t=Zper7b85RVlpWoTKKJDkbg",
        );
        assert_eq!(
            val,
            "Here is some things from twitter.com https://nitter.net/wezm/status/1323096439602339840 ([source](https://twitter.com/wezm/status/1323096439602339840?s=20&t=Zper7b85RVlpWoTKKJDkbg)) and https://nitter.net/rustlang/status/1496894318887546883 ([source](https://twitter.com/rustlang/status/1496894318887546883?s=20&t=Zper7b85RVlpWoTKKJDkbg))",
        );
    }

    #[test]
    fn twitter_to_nitter_invalid() {
        let val = substitute_urls("https://twitter");
        assert_eq!(val, "https://twitter");
    }

    #[test]
    fn x_tweet_to_nitter() {
        let val = substitute_urls(
            "https://x.com/nealagarwal/status/1691095252952834048?s=46&t=OJUN8AoB2f1zmJVHufidVg",
        );
        assert_eq!(
            val,
            "https://nitter.net/nealagarwal/status/1691095252952834048 ([source](https://x.com/nealagarwal/status/1691095252952834048?s=46&t=OJUN8AoB2f1zmJVHufidVg))",
        );
    }

    #[test]
    fn medium_to_scribe() {
        let val = substitute_urls(
        "https://medium.com/swlh/make-your-raspberry-pi-file-system-read-only-raspbian-buster-c558694de79",
    );
        assert_eq!(
            val,
            "https://scribe.rip/swlh/make-your-raspberry-pi-file-system-read-only-raspbian-buster-c558694de79 ([source](https://medium.com/swlh/make-your-raspberry-pi-file-system-read-only-raspbian-buster-c558694de79))",
        );
    }

    #[test]
    fn medium_to_scribe_subdomain() {
        let val = substitute_urls(
            "https://jxxcarlson.medium.com/lambda-calculus-an-elm-cli-fd537071db2b",
        );
        assert_eq!(
            val,
            "https://scribe.rip/lambda-calculus-an-elm-cli-fd537071db2b ([source](https://jxxcarlson.medium.com/lambda-calculus-an-elm-cli-fd537071db2b))",
        );
    }

    #[test]
    fn substitute_urls_mixed() {
        let val = substitute_urls(
        "Here are some things from twitter.com https://twitter.com/wezm/status/1323096439602339840?s=20&t=Zper7b85RVlpWoTKKJDkbg and Medium https://jxxcarlson.medium.com/lambda-calculus-an-elm-cli-fd537071db2b",
        );
        assert_eq!(
            val,
            "Here are some things from twitter.com https://nitter.net/wezm/status/1323096439602339840 ([source](https://twitter.com/wezm/status/1323096439602339840?s=20&t=Zper7b85RVlpWoTKKJDkbg)) and Medium https://scribe.rip/lambda-calculus-an-elm-cli-fd537071db2b ([source](https://jxxcarlson.medium.com/lambda-calculus-an-elm-cli-fd537071db2b))",
        );
    }
}
