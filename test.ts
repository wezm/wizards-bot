import {
  assert,
  assertEquals,
  assertMatch,
} from "https://deno.land/std@0.127.0/testing/asserts.ts";
import { substituteUrls } from "./src/url.ts";

Deno.test("twitterToNitter desktop", () => {
  let val = substituteUrls("https://twitter.com/wezm");
  assertEquals(
    val,
    "https://nitter.net/wezm ([source](https://twitter.com/wezm))",
  );
});

Deno.test("twitterToNitter mobile", () => {
  let val = substituteUrls(
    "https://mobile.twitter.com/wezm/status/1323096439602339840?s=20&t=Zper7b85RVlpWoTKKJDkbg",
  );
  assertEquals(
    val,
    "https://nitter.net/wezm/status/1323096439602339840?s=20&t=Zper7b85RVlpWoTKKJDkbg ([source](https://mobile.twitter.com/wezm/status/1323096439602339840?s=20&t=Zper7b85RVlpWoTKKJDkbg))",
  );
});

Deno.test("twitterToNitter multiple", () => {
  let val = substituteUrls(
    "Here is some things from twitter.com https://twitter.com/wezm/status/1323096439602339840?s=20&t=Zper7b85RVlpWoTKKJDkbg and https://twitter.com/rustlang/status/1496894318887546883?s=20&t=Zper7b85RVlpWoTKKJDkbg",
  );
  assertEquals(
    val,
    "Here is some things from twitter.com https://nitter.net/wezm/status/1323096439602339840?s=20&t=Zper7b85RVlpWoTKKJDkbg ([source](https://twitter.com/wezm/status/1323096439602339840?s=20&t=Zper7b85RVlpWoTKKJDkbg)) and https://nitter.net/rustlang/status/1496894318887546883?s=20&t=Zper7b85RVlpWoTKKJDkbg ([source](https://twitter.com/rustlang/status/1496894318887546883?s=20&t=Zper7b85RVlpWoTKKJDkbg))",
  );
});

Deno.test("twitterToNitter invalid", () => {
  let val = substituteUrls("https://twitter");
  assertEquals(val, "https://twitter");
});

Deno.test("mediumToScribe", () => {
  let val = substituteUrls(
    "https://medium.com/swlh/make-your-raspberry-pi-file-system-read-only-raspbian-buster-c558694de79",
  );
  assertEquals(
    val,
    "https://scribe.rip/swlh/make-your-raspberry-pi-file-system-read-only-raspbian-buster-c558694de79 ([source](https://medium.com/swlh/make-your-raspberry-pi-file-system-read-only-raspbian-buster-c558694de79))",
  );
});

Deno.test("mediumToScribe subdomain", () => {
  let val = substituteUrls(
    "https://jxxcarlson.medium.com/lambda-calculus-an-elm-cli-fd537071db2b",
  );
  assertEquals(
    val,
    "https://scribe.rip/lambda-calculus-an-elm-cli-fd537071db2b ([source](https://jxxcarlson.medium.com/lambda-calculus-an-elm-cli-fd537071db2b))",
  );
});

Deno.test("substituteUrls mixed", () => {
  let val = substituteUrls(
    "Here are some things from twitter.com https://twitter.com/wezm/status/1323096439602339840?s=20&t=Zper7b85RVlpWoTKKJDkbg and Medium https://jxxcarlson.medium.com/lambda-calculus-an-elm-cli-fd537071db2b",
  );
  assertEquals(
    val,
    "Here are some things from twitter.com https://nitter.net/wezm/status/1323096439602339840?s=20&t=Zper7b85RVlpWoTKKJDkbg ([source](https://twitter.com/wezm/status/1323096439602339840?s=20&t=Zper7b85RVlpWoTKKJDkbg)) and Medium https://scribe.rip/lambda-calculus-an-elm-cli-fd537071db2b ([source](https://jxxcarlson.medium.com/lambda-calculus-an-elm-cli-fd537071db2b))",
  );
});
