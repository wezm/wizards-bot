import { assert, assertEquals, assertMatch } from "https://deno.land/std@0.127.0/testing/asserts.ts";
import { twitterToNitter, mediumToScribe } from "./src/url.ts";

Deno.test("twitterToNitter desktop", () => {
    let val = twitterToNitter("https://twitter.com/wezm")
    assertEquals(val, "https://nitter.net/wezm")
});

Deno.test("twitterToNitter mobile", () => {
    let val = twitterToNitter("https://mobile.twitter.com/wezm/status/1323096439602339840?s=20&t=Zper7b85RVlpWoTKKJDkbg")
    assertEquals(val, "https://nitter.net/wezm/status/1323096439602339840?s=20&t=Zper7b85RVlpWoTKKJDkbg")
});

Deno.test("twitterToNitter multiple", () => {
    let val = twitterToNitter("Here is some things from twitter.com https://twitter.com/wezm/status/1323096439602339840?s=20&t=Zper7b85RVlpWoTKKJDkbg and https://twitter.com/rustlang/status/1496894318887546883?s=20&t=Zper7b85RVlpWoTKKJDkbg")
    assertEquals(val, "Here is some things from twitter.com https://nitter.net/wezm/status/1323096439602339840?s=20&t=Zper7b85RVlpWoTKKJDkbg and https://nitter.net/rustlang/status/1496894318887546883?s=20&t=Zper7b85RVlpWoTKKJDkbg")
});

Deno.test("twitterToNitter invalid", () => {
    let val = twitterToNitter("https://twitter")
    assertEquals(val, "https://twitter")
});

Deno.test("mediumToScribe", () => {
    let val = mediumToScribe("https://medium.com/swlh/make-your-raspberry-pi-file-system-read-only-raspbian-buster-c558694de79")
    assertEquals(val, "https://scribe.rip/swlh/make-your-raspberry-pi-file-system-read-only-raspbian-buster-c558694de79")
});

Deno.test("mediumToScribe subdomain", () => {
    let val = mediumToScribe("https://jxxcarlson.medium.com/lambda-calculus-an-elm-cli-fd537071db2b")
    assertEquals(val, "https://scribe.rip/lambda-calculus-an-elm-cli-fd537071db2b")
});

