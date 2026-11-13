# Jauge

Your live Claude limits in the menu bar: a ring and the percentage of your 5-hour session. A click opens the details, a right click opens the menu (refresh, open at login, quit). No window, no Dock icon.

Built with Tauri: the back end in Rust (`src-tauri/`), the panel in HTML, CSS and JavaScript (`ui/`), rendered by the WebKit of macOS. About 90 MB of memory in total, against 195 MB for the old Electron version.

## What it shows

- **5-hour session, week, and per-model limits** (Fable): percentage, reset time, countdown.
- **The pace, as in Fil:** the white part of each bar is what you have used, the tick is the time elapsed in the window. Between the two, a veil: green when you have room to spare, yellow when you are ahead of the clock, red at 15 points ahead or more.
- **The API equivalent:** what your usage would have cost at API prices, since the first message read in your logs, and today's share.

The numbers stay neutral: only the bar carries colour. The menu bar ring takes the colour of the session's veil (orange instead of yellow, more readable at the top of the screen).

## Where the numbers come from

- **Limits:** the Claude binary shipped with the Claude Agent SDK (0.3.319, pinned in `package.json`), using the Claude Code sign-in of the computer. Jauge sends it the same request as the SDK (`get_usage`, marked experimental): check it again if you update the SDK. Read every minute, and as soon as Claude writes to its logs (every 20 s at most), when the Mac wakes up, when you come back and when the panel opens. With the Mac locked or you away for 5 min: one reading every 5 min. At the reset time, the limit starts again from 0 without waiting for a reading. With no successful reading for 10 min, the ring turns grey.
- **API equivalent:** the logs in `~/.claude/projects`, at the public prices as of 06/10/2026 in `src-tauri/src/cout.rs` (to be updated if Anthropic changes its prices), converted to euros at the ECB rate of the day. Only the logs that change are reread; the read position is kept in `~/Library/Application Support/Jauge/equivalent-api.json`, which avoids rereading everything at each start. A log deleted by Claude Code is still counted.

## Install or rebuild (macOS)

You need Node.js 22, Rust and the Xcode tools.

```bash
npm install
```

```bash
npm run install-app
```

The second command runs the tests, builds `Jauge.app`, checks it, places it in `/Applications` and opens it. On the first launch, it registers itself to open at login (it can be unchecked in the right-click menu).

## Linux (WSL)

Jauge also runs on Linux, including WSL2 with WSLg.

Requirements:

- Node.js 22
- Rust, installed with [rustup](https://rustup.rs)
- The system libraries:

```bash
sudo apt install pkg-config build-essential libssl-dev libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev
```

Then:

```bash
npm install
```

```bash
npm run install-app:linux
```

The second command runs the tests, builds Jauge, installs it in `~/.local/opt/jauge` with the SDK's Claude binary next to it, and registers a `.desktop` entry, which WSLg also lists in the Windows Start menu.

What differs from the Mac version:

- There is no tray (WSLg has none): the panel is a normal window, and closing it quits Jauge.
- There is no idle detection, so Jauge always counts as present and reads every minute.
- There is no "open at login".
- Data lives in `~/.local/share/jauge`.
- The Claude sign-in is the one Claude Code keeps in `~/.claude`.

## If the icon does not appear (macOS)

On the first launch, Jauge places itself next to the battery, on the visible side of the notch (`PLACE_BARRE` in `src-tauri/src/macos.rs`). On a Mac with a notch, the bar does not have room for everything: another icon then goes behind the chevron "<<". To change Jauge's position, drag the ring while holding ⌘; macOS remembers it.

## Testing

`npm test` checks the panel calculations (JavaScript) and the back end (Rust). `JAUGE_ESSAI=<folder> JAUGE_DEMO=1 src-tauri/target/release/jauge` captures the panel with fake data; without `JAUGE_DEMO`, with your real numbers.
