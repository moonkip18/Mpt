# Mpt

A small desktop study timer for [Yeolpumta](https://yeolpumta.com) (YPT), written in Rust.

Sign in with the account you already study on, and the sessions you run on the PC
land in the same log as your phone. No group chat, no rankings, no planner — a
clock, your subjects, today's totals, and a stop button that survives a dead
network.

## What it does

**Timer.** Start, pause, stop. Pause is YPT's *rest*, so a break you take here is
also a break on your phone. The running session is snapshotted to disk, so if the
app crashes or you reboot mid-session, reopening restores it — including whether
it was paused and how much rest had accumulated.

**Subjects.** Your YPT subject list, in your order, with your colours. Click a
chip to switch; the `›` control opens the full list with today's time beside each
subject. While a session is running the list is locked, so a stray click cannot
move minutes to the wrong subject.

**Today by subject.** A panel that is always visible: every subject studied
today, its hours, and a thin bar sized against your biggest subject. The live
remainder of a running session is folded in, and sessions logged on the phone
count too.

**Offline queue.** A stop that cannot reach YPT is saved locally and replayed
later — on the next sign-in, or with the *sync now* link in the panel. A
duplicate guard asks the server whether that session is already there before
posting, so a lost response does not become a double entry.

**Mini player.** Shrink the window to 360×148, pinned above other windows:
subject, clock, start/pause, stop. Same process, same session — just small.

## Install

Grab `Mpt-windows-x64.zip` from the [releases page](https://github.com/moonkip18/Mpt/releases),
unzip it anywhere, run `mpt.exe`. Settings and the session snapshot live in
`%APPDATA%\Mpt`; delete that folder to reset the app.

## Build

Rust 1.85 or newer (the crate uses edition 2024).

```sh
cargo build --release
```

The exe lands at `target/release/mpt.exe`. On Windows the MSVC toolchain is the
path of least resistance; `cargo check` is enough if you only want to confirm the
code compiles.

CI builds every push and attaches the zip to tagged releases, so you do not need
a local toolchain to get a binary — see `.github/workflows/build.yml`. Every run
keeps the zip under *Actions → build → Artifacts*; if you want the release page
as well, set *Settings → Actions → General → Workflow permissions* to
**Read and write permissions**, then push a `v*` tag.

## How the hours stay honest

YPT's `/logs/day` returns per-subject totals *and* the raw session list. The
original client added both, which counts every subject twice. Here the payload is
parsed once, in `src/daily_stats.rs`:

- Each list is summed on its own; the larger total wins per subject. A session
  stored in only one of them still shows, one stored in both is counted once.
- A dated request that comes back empty is retried without the date, because YPT
  buckets the day in the account's timezone, which may not be yours.
- The panel refreshes every 30 seconds, when the window regains focus, and after
  every stop.
- The running session is added as a *delta* (`elapsed − last synced`), so a pause
  that already reached the server is never counted twice.

## Where things live

| File | Job |
| --- | --- |
| `src/main.rs` | window, frame loop, closing the app cleanly |
| `src/ui/` | login screen, study screen, mini player |
| `src/timer.rs` | the session state machine, on its own thread |
| `src/repositories/` | the YPT calls: auth, subjects, timer, offline queue |
| `src/daily_stats.rs` | turning `/logs/day` into per-subject totals |
| `src/state.rs` | one shared snapshot the UI and the workers both read |
| `src/background.rs` | the quiet drifting backdrop |
| `src/store.rs` | `%APPDATA%\Mpt\prefs.json` |

Network work never runs on the UI thread: auth, the timer, and the offline queue
each own a worker that reports back through the shared snapshot.

## Known limits

- YPT's API is private and undocumented, so a server change can break the client.
- Phone → PC session mirroring is read-only on purpose. Adopting a session that
  started on the phone and then stopping it would make YPT count those minutes
  twice, so you get a `PHONE LIVE` badge and live totals instead.
- No auto-update, no tray icon, no second-instance forwarding.

## Credit

This is a rewrite of the earlier Flutter client for the same app, which is still
on the `flutter` branch. The visual language (near-black ground, hairlines, one
red accent) came from that client, which in turn grew out of the open-source
[Desky](https://github.com/huneyoliv/desky) project.

Unofficial client. Not affiliated with Yeolpumta.
