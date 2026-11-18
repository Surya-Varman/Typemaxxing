# typemaxxing

A fast typing speed test for your terminal. Fully offline: the word list is built into the program.

## Install

**macOS / Linux**

```sh
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/Surya-Varman/Typemaxxing/releases/latest/download/typemaxxing-installer.sh | sh
```

**Windows (PowerShell)**

```powershell
powershell -ExecutionPolicy Bypass -c "irm https://github.com/Surya-Varman/Typemaxxing/releases/latest/download/typemaxxing-installer.ps1 | iex"
```

**With Rust installed**

```sh
cargo install typemaxxing
```

Or download a ready-made build from the [releases page](https://github.com/Surya-Varman/Typemaxxing/releases). Then run `tmx`.

## Run

```sh
tmx              # test with your last-used settings
tmx -t 60        # 60-second test
tmx -w 25        # type 25 words
tmx history      # past results and averages
```

From a clone of this repo, `cargo run --release -- <options>` does the same, and `cargo install --path .` installs the `tmx` command.

## Keys

Commands start with `/` and work any time. Single letters run immediately; longer commands run once the full name is typed (case doesn't matter; Backspace edits, Esc cancels). Changing mode or length starts a new test.

| Keys | Action |
|---|---|
| `/m` | toggle mode: time limit or word count |
| `/s` / `/S` | change the time limit (15, 30, 60, 120 s) or length (10, 25, 50, 100 words), forward / back |
| `/onePressQuit` | quit with a single Esc, no "Quit typemaxxing?" box: on / off (off by default) |
| `/fontSize` | bigger or smaller practice text: ↑ / ↓ (or + / −), Enter to close. Sizes 2–10 draw letters from block characters |
| `/color` | colour theme: purple (default), pink or yellow. ← / → to choose, Enter to close |
| `/h` | help |

While typing:

| Key | Action |
|---|---|
| Backspace | delete a character |
| Option/Ctrl+Backspace, Ctrl+W | delete a word |
| Tab | mid-test: pause (Tab again for the next test, any other key to resume). Before starting / on results: new words |
| Enter (on results) | next test |
| Esc | quit (asks first unless one-press quit is on; Esc again to confirm, any other key to keep going) |
| Ctrl+C | quit immediately |

Your settings are remembered between runs. `-t` and `-w` on the command line override them.

## How it scores

- **WPM** (Monkeytype-style): characters in words typed completely correctly, plus the space after each, ÷ 5, per minute. One wrong letter left in a word means that word earns nothing. It's judged on the final text, so a mistake fixed with Backspace before the end costs nothing. The word you're in when time runs out counts if it's correct so far.
- **Raw**: all typed characters ÷ 5, per minute
- **Accuracy**: share of keystrokes that were correct (mistakes you fixed still count here)

Results and settings are saved in `~/Library/Application Support/typemaxxing/` on macOS (`~/.local/share/typemaxxing/` on Linux). Data from the app's old name, typetest, is copied over automatically.

## License

MIT, see [LICENSE](LICENSE).

## Layout

```
src/main.rs     CLI flags, terminal setup/restore
src/settings.rs mode/length options, saved between runs
src/app.rs      event loop, key handling, saving results
src/typing.rs   test state: target text, typed text, timer
src/stats.rs    WPM / accuracy math
src/ui.rs       ratatui drawing and line layout
src/history.rs  result history (JSON Lines)
src/paths.rs    where data is stored (and migration from the old name)
src/bigtext.rs  big block letters for /fontSize
src/words.rs    random words from assets/english.txt
```

Run the tests with `cargo test`.


## Screenshots
<img width="1602" height="880" alt="image" src="https://github.com/user-attachments/assets/bf1f16c2-3129-4048-ae6c-4b2b595c98af" />
<img width="1499" height="760" alt="Screenshot 2026-11-18 at 3 12 44 PM" src="https://github.com/user-attachments/assets/ea80927e-571c-44b8-a6a9-bcabf0bda4eb" />
<img width="972" height="640" alt="Screenshot 2026-11-18 at 3 13 26 PM" src="https://github.com/user-attachments/assets/5f4b6cdf-c49e-4365-badc-8a9c46151ee6" />



