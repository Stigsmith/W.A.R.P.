// Builds the installer (`npm run release`) with the builder's folders kept out of it.
//
// Rust embeds the source paths of every dependency in the program for its error
// messages, e.g. C:\Users\<name>\.cargo\registry\...\lib.rs. Remapping the home
// folder and the repo folder keeps a user name out of a published build, and the
// check at the end refuses a build that still contains one.
import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { homedir, userInfo } from "node:os";
import { join, resolve } from "node:path";

const root = resolve(import.meta.dirname, "..", "..");
const flags = [`--remap-path-prefix=${homedir()}=~`, `--remap-path-prefix=${root}=warp`];
// CARGO_ENCODED_RUSTFLAGS separates flags with 0x1f, so folders with spaces survive.
const encoded = [process.env.CARGO_ENCODED_RUSTFLAGS, ...flags].filter(Boolean).join("\x1f");
const env = { ...process.env, CARGO_ENCODED_RUSTFLAGS: encoded };
delete env.RUSTFLAGS;

// One command string: Windows runs npx through a shell, and Node warns when args are passed separately.
const build = spawnSync(["npx", "tauri", "build", ...process.argv.slice(2)].join(" "), {
  cwd: resolve(import.meta.dirname, ".."),
  env,
  stdio: "inherit",
  shell: true,
});
if (build.status !== 0) process.exit(build.status ?? 1);

const exe = readFileSync(join(root, "target", "release", "warp-app.exe")).toString("latin1").toLowerCase();
const leaks = [homedir(), userInfo().username].filter((s) => s.length >= 3 && exe.includes(s.toLowerCase()));
if (leaks.length > 0) {
  console.error(`\nrelease: warp-app.exe still contains ${leaks.length === 1 ? "a personal path" : "personal paths"}. Don't publish this build.`);
  process.exit(1);
}
console.log("\nrelease: no personal paths in warp-app.exe.");
