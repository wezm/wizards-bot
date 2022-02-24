import { assert, assertEquals, assertMatch } from "https://deno.land/std@0.127.0/testing/asserts.ts";
import { twitterToNitter } from "./src/url.ts";

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


// Deno.test("pick", () => {
//     const array = [1, 2, 3];
//     const val = pick(array);
//     assert(array.indexOf(val) >= 0);
// });

// Deno.test("mistakeText", () => {
//     assertEquals(mistakeText('emoji'), "😀 Emoji were a mistake");
//     assertEquals(mistakeText('invalid'), "Breaking URLs was a mistake");
// });

