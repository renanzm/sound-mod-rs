# Sound Mod (Rust) — Architecture and Development Guide

**English** | [Português](ARQUITETURA.md)

Windows soundboard that plays sounds on the microphone channel, with global hotkeys and recording of the last N seconds of PC audio. A Rust rewrite of the Python Sound Mod, with the same features and its own neon interface.

## 1. Overview

```
Real microphone ──┐
                  ├─► [Engine: 48 kHz mixer] ─► CABLE Input ═► CABLE Output ─► Discord/game (microphone)
Sounds (F1..F4) ──┘          │
                             └─► Headphones (monitor, only you hear it)

PC audio ─► WASAPI loopback ─► ring buffer (N s) ─► F5 writes WAV ─► F6 plays it as a sound
```

The app depends on VB-CABLE. Without it the interface warns and the mixer has nowhere to send audio.

### Stack

| Purpose | Crate |
|---|---|
| Interface (immediate mode, OpenGL) | `eframe` / `egui` 0.37 |
| WASAPI audio I/O (mic, cable, headphones, loopback) | `cpal` 0.18 |
| Decoding mp3/wav/ogg/flac/m4a/aac/aiff... | `symphonia` 0.5 (pure Rust) |
| FFT for the Robot and Autotune effects | `realfft` |
| Writing WAV | `hound` |
| File picker | `rfd` |
| Keyboard hook, language, title bar | `windows-sys` |
| Config | `serde` / `serde_json` |

Everything is pure Rust: there is no C code to compile.

## 2. Layout

```
sound-mod-rs/
├── build.ps1               # builds (release into dist\SoundMod), runs, tests
├── build.rs                # embeds icon + version in the .exe (writes the .res without rc.exe)
├── rust-toolchain.toml     # stable GNU + llvm-tools
├── .cargo/config.toml      # rust-lld linker
├── docs/                   # this guide (en/pt) and the screenshots
└── src/
    ├── main.rs             # window (eframe), panic log
    ├── config.rs           # config.json, folders (%APPDATA%\SoundMod), import from older versions
    ├── library.rs          # sound order and collections (pure functions, tested)
    ├── i18n.rs             # pt/en texts, t()/tf(), Windows language
    ├── codec.rs            # decode, resample, write WAV
    ├── engine.rs           # mixer, channels, cpal streams, devices
    ├── voice.rs            # real-time voice effects
    ├── recorder.rs         # loopback + buffer of the last N s
    ├── hotkeys.rs          # WH_KEYBOARD_LL hook, combos, key capture
    ├── logic.rs            # Core: app actions (shared by the UI and the hotkeys)
    ├── icon_raster.rs      # the icon drawn in pixels (window and .exe)
    └── ui/
        ├── app.rs          # main window (SoundApp)
        ├── theme.rs        # palettes, fonts, style, neon glow, themes
        ├── widgets.rs      # glowing buttons, key button, switch, slider, meter, card
        ├── waveform.rs     # waveform with handles
        ├── icons.rs        # vector icons
        ├── logo.rs         # header logo and window icon
        └── window.rs       # title bar and border in the neon color (DWM, Windows 11)
```

## 3. Modules

### 3.1 `config.rs`
- **Folders:** `Dirs { app, data }`. `app` = the `.exe` folder (when running from `target\debug|release`, the project root). `data` = `%APPDATA%\SoundMod`, holding `config.json`, `sounds\` and `clips\last_clip.wav`: replacing the program folder in an update erases nothing. It is the **same folder as the Python version**, which reads the same file.
- **One-time import:** if `data` has no `config.json` yet and there is one next to the `.exe` (older versions), it copies `sounds\` and `clips\` without overwriting anything that already exists and, last, the `config.json` (if something fails midway, the next start tries again). Nothing is deleted from the old folder. `load()` returns the source folder and the interface says so in the status bar.
- **Corrupted config:** copied to `config-corrompido-YYYYMMDD-HHMMSS.json` before falling back to defaults (otherwise the next save would overwrite it). A Notepad BOM is accepted.
- Same format as the Python version: `language`, `mic_device`, `output_device`, `monitor_device`, `loopback_device`, `mix_mic`, `monitor`, `master_volume`, `clip_seconds`, `voice_effect`, `accent`, `hear_self`, `clip_trim`, `save_clip_key`, `play_clip_key`, `stop_key`, `collections[{name, collapsed}]`, `slots[{key, path, volume, collection}]`. **Unknown fields are preserved** (`#[serde(flatten)] extra`) so one version doesn't erase what the other wrote. `collections` also accepts bare names (`["Memes"]`).
- Devices by **name**: `None` = automatic (resolved on first start, then saved), `Some("")` = none. The old text `"(nenhum)"` becomes `""`.
- `resolve()`: relative paths (`sounds/x.wav`) are relative to the data folder.
- `save()` writes `config.tmp` and renames it.
- `Slot.id` only exists in memory: it is the channel id in the mixer (and identifies the sound for order/collections, since two empty slots have equal contents).
- `load_in`/`save_in` take the folders: tests use temporary folders.
- `peek_accent()` reads only the saved color, so the window is created with the right icon.

### 3.1.1 `library.rs` (order and collections)
Pure functions over the `Config`, ported from `library.py`:
- The order of `cfg.slots` is the on-screen order. Sounds without a collection come first, then each collection in `cfg.collections` order (`groups()`).
- `move_slot(id, ±1)`: swaps with the neighbor **in the same collection**. `set_collection(id, name)`: puts the sound at the end of the collection (error if it doesn't exist).
- `clean_name()`: collapses extra spaces; error if empty or already taken (case-insensitive; a rename may change only the case); the "(sem coleção)"/"(no collection)" labels of every language are reserved.
- `rename_collection`, `delete_collection` (its sounds go to the end of "no collection"; no sound is deleted), `move_collection`, `set_collapsed`.
- `normalize()`: fixes old or hand-edited configs (blank names, duplicate collections, a sound pointing to a missing collection, which is created).

### 3.2 `codec.rs`
- Internal format: interleaved stereo `Vec<f32>` at **48 kHz**.
- `decode()`: Symphonia → stereo (mono is duplicated; multichannel uses front + center) → `resample()` to 48 kHz. If Symphonia can't open it, uses `ffmpeg.exe` (next to the program or on PATH) without a console window.
- `resample()`: polyphase windowed sinc (512 phases, Blackman), with a lower cutoff when downsampling.
- `write_wav()`: 16-bit PCM.

### 3.3 `engine.rs`
`Engine` is shared (`Arc`) by the interface, the hotkeys and the callbacks. The `cpal::Stream` objects belong to the interface (`SoundApp`); closing = dropping them (`Engine::close`).

**Streams:**
1. Microphone: device's native format → mono → `LinearResampler` → `mic_buf`.
2. Output to the cable: **48 kHz stereo f32** requested directly; cpal uses `AUTOCONVERTPCM`, so WASAPI converts to the device format. If stereo fails, it tries the native channel count.
3. Monitor (optional): also 48 kHz; consumes `mon_buf`.

**`render` (output callback):** sums the playing channels × gain × master volume; pulls the mic and applies the voice effect (under `catch_unwind`: a faulty effect is turned off instead of killing the stream); feeds the monitor (sounds + voice if `hear_self`); mixes the mic if `mix_mic`; clamps to [-1, 1]; publishes peaks for the meters.

**Channels:** `Channel { data, pos, start, end, gain, playing }` per `ChannelId` (`CLIP` = 0, slots ≥ 1). Only the `[start, end)` range plays.

| Method | Effect |
|---|---|
| `set_sound(id, path, gain)` | decodes (cached) and creates the channel, stopped; returns the duration |
| `play(id)` | restarts if already playing; otherwise continues from `pos` |
| `toggle(id)` | playing → stops; otherwise plays. `None` if the channel doesn't exist |
| `pause(id)` | pause/resume (stopped at the start: nothing) |
| `stop` / `stop_all` | stops and rewinds to the start of the range |
| `seek`, `set_gain`, `set_range` | position, live volume, trim |
| `state(id)` | `(pos_s, duration_s, playing)` |
| `channel_audio(id)` | data + range (waveform, save as sound) |

**One sound at a time:** `play`, `toggle` and resuming a pause call `solo`, which stops the other channels.
**Latency:** `mic_buf` and `mon_buf` drop anything beyond 250 ms (device clocks drift).
**Device errors** (disconnected) set `device_lost`; the interface asks you to click Apply.

### 3.4 `voice.rs`
Direct port of `voice.py`, now sample by sample (no numpy). Each effect implements `Effect::process(&mut [f32])` and keeps state across blocks of any size.

| Effect | How it works |
|---|---|
| `deep` / `high` | `PitchShifter` (0.75 / 1.45): delay line read by two cursors in antiphase with sin² windows |
| `ogre` | pitch 0.62 + `tanh` saturation + 2.2 kHz FIR low-pass |
| `robot` | 1024/256 STFT with zeroed phase (monotone voice at ~188 Hz) |
| `autotune` | FFT autocorrelation over 2048 samples + 0.6-semitone hysteresis |
| `echo` | 280 ms, feedback 0.45 |
| `radio` | 400–3000 Hz FIR band-pass (481 taps) + saturation |
| `alien` | pitch 1.3 with a 5 Hz vibrato |

**New effect:** create the type, register it in `make()` and `EFFECTS`, and add the `fx_<name>` key to `logic::FX_KEYS` and to both languages in `i18n`.

### 3.5 `recorder.rs`
- In WASAPI, an **input** stream on an **output** device turns on loopback. It uses the native format (rate and channels) and keeps samples in a `VecDeque` limited to `seconds × rate × channels`.
- `save()` writes the last N s as WAV at the native rate. With no audio playing on the PC, no samples arrive ("Nothing recorded yet").

### 3.6 `hotkeys.rs`
- `WH_KEYBOARD_LL` hook on a thread with a message loop. The hook only forwards `(key, pressed?)` through a channel and returns immediately (Windows removes slow hooks). It doesn't block the key: it still reaches the game.
- Worker thread: ignores auto-repeat, builds the combo (`ctrl+shift+a`, `f1`), checks "stuck" modifiers with `GetAsyncKeyState` and fires the registered actions.
- `begin_capture()`: the next key goes to the interface instead of firing actions; **Esc cancels**. Pressing only a modifier keeps waiting.
- Stable lowercase names; `normalize()` accepts old synonyms (`control`, `escape`, `page_up`...).

### 3.7 `logic.rs` (`Core`)
What the interface and the hotkeys do: play/stop slots, save/play the clip, save the selection as a sound, add/remove sounds (also straight into a collection: uses the first empty slot **of the same group** or creates one at its end, and expands the collection), change the order, create/rename/delete/move/collapse collections (validating the name with an already-translated message), change keys, change the effect, save the config. Hotkeys call `Core::on_action` **on their own thread**, so they work with the window minimized (eframe doesn't even call `ui()` then). Slow work goes to threads; messages go to `status`, which the interface reads every frame.

### 3.8 Interface (`ui/`)
- **Immediate mode:** `SoundApp::ui` redraws everything every frame (~30 fps, because of the animations) reading the state of `Core`/`Engine`. Row actions are collected into a `Vec<RowAction>` and applied after drawing.
- **Layout:** top panel (logo, neon title, Live, meters, color, language, Stop all), status bar at the bottom (with the **Data folder** button), fixed side panel (Devices, Voice effect) and the central area with the clip in a bottom panel and the sounds filling the rest.
- **Sound list:** drawn per group (`library::groups`) from a copy of the config. With no collections there are no headers. Each collection has a header (arrow/name collapse, **+ Sound**, ▲▼, ✎, trash); a collapsed collection doesn't draw its rows, but its keys keep working. Each row has stacked ▲▼ (disabled at the group's edges) and a layers button with the collection menu (`egui::Popup::menu`), including "+ New collection", which also moves the sound into it.
- **Name dialog** (`NameDialog`): save clip as sound, new collection and rename collection; validation errors show inside it, in red, without closing.
- **Neon (`theme.rs`):** the glow is a colored `Shadow` with no offset behind the element (`glow`); neon text is the same galley repainted with a transparent halo. Fixed colors: only the neon red for remove/stop all.
- **Base colors:** `Base` groups background, panels, items, borders, texts, wells, tracks and headers. There are two: `GRAPHITE` (neon colors) and `DUSK` (night violet, for the Sunset theme). The interface never uses one of these colors directly: it calls `bg()`, `panel()`, `item()`, `border()`, `txt()`, `muted()`, `dim()`, `well()`, `track()`, `header_fill()`, which follow the current choice.
- **Neon color:** `ACCENTS` has 8 colors (green, cyan, blue, purple, pink, red, orange, yellow), each with the tube (`neon`) and the light core (`core`). The current one lives in an atomic; the interface never uses a fixed accent, only `accent()`, `accent_core()`, `accent_deep()` (tinted background) and `on_accent()` (text on the neon: dark on light colors, white on dark ones, by luminance). Changing the color (`SoundApp::set_accent`) calls `set_accent`, reapplies the egui style, recolors the window border (`window::recolor`), swaps the window icon (`ViewportCommand::Icon`) and saves `accent` to `config.json`. **New color:** add it to `ACCENTS` and the `accent_<name>` key in both languages and in `i18n::accent_name`.
- **Sunset theme:** the `sunset` entry of `ACCENTS` uses `DUSK`, the Coral Pink neon and the `SUNSET` gradient (Magenta, Coral Pink, Coral, Orange, Warm Yellow), with violet text on the neon. `spectrum_at(t)` gives the gradient color at a position (without a gradient, the neon color); `fill()` paints a rounded rect with a 256x1 gradient texture (created in `apply_style`), used by the primary buttons, the playing key, the selected effect and the slider tracks. The title (letter by letter), waveform, meters, background lights and icon bars use `spectrum_at`. **New theme:** an `ACCENTS` entry with `base` and `spectrum`.
- **Icon:** the 5 bars each have their own color (`icon_raster::BAR_COLORS`, rainbow; in the theme, `theme::bar_colors()` uses the palette) and the frame follows the neon. The `.exe` uses a green frame and rainbow bars; the window starts with the saved color (`config::peek_accent` in `main.rs`).
- **Fonts:** Windows' Segoe UI, Segoe UI Semibold and Bahnschrift (titles) when present; otherwise egui's. Icons are vector-drawn (`icons.rs`), no icon font.
- **Custom widgets:** `button` (glowing pill), `key_button` (pill + ✎ + ⌫, with its own layout so it works inside right-to-left layouts), `toggle`, `slider`, `level_meter`, `status_pill`, `choice`, `card` (with a light strip on top), `accent_button`/`accent_option`, `Waveform`.
- **Window:** border in the neon color and a dark title bar via `DwmSetWindowAttribute` (Windows 11; ignored on 10).

### 3.9 Languages (`i18n.rs`)
- `TEXTS` tables `PT` and `EN` with the same keys; `t("key")` and `tf("key", &[("name", &value)])` return the text in the current language. A test ensures both languages have the same keys and the same `{fields}`.
- `language` in the config: `"pt"`, `"en"` or `null` = Windows language (`GetUserDefaultUILanguage`: Portuguese → pt, anything else → en). The header menu switches it live and saves it.
- **New language:** add it to `LANGUAGES` and create the table with the same keys. **New text:** never write it in the code; create the key in every language.

## 4. Threads

| Thread | What it does | Rule |
|---|---|---|
| Main (eframe) | interface, opening/closing streams | owns the `cpal::Stream`s |
| cpal callbacks | mic, output, monitor, loopback | fast, no I/O or UI |
| `keyboard-hook` | hook + message loop | only forwards events |
| `hotkeys` | combos and actions (`Core::on_action`) | heavy work goes to threads |
| Short workers | decoding, writing WAV | report through `Core::say` |

## 5. Building (why GNU + rust-lld)

- The **GNU** toolchain doesn't require Visual Studio. `rust-toolchain.toml` pins `stable-x86_64-pc-windows-gnu` with `llvm-tools`.
- The `windows*` crates import functions through `raw-dylib`, which needs a **dlltool**. The one in the GNU package needs the `as` assembler, which isn't included; `build.ps1` uses `llvm-ar` from `llvm-tools` renamed to `dlltool.exe` (it then acts as `llvm-dlltool`).
- The bundled **GNU ld** links those import libraries incorrectly: the program starts and crashes with an access violation (a jump to an unresolved import address). With **rust-lld** (`.cargo/config.toml`) linking is correct.
- `build.rs` builds the `.res` (16–256 px icon + VERSIONINFO) in Rust and hands it to the linker, without `rc.exe`/`windres`.

## 6. Tests

```
.\build.ps1 -Test              # unit tests
.\build.ps1 -Test -Hardware    # + real test with VB-CABLE
```

- `codec`: resampling (length and frequency), WAV round trip.
- `engine`: play/stop/solo, mixing and end of range, pause/seek/range, mic → output and monitor, latency cap, linear resampler rate.
- `voice`: pitch-shifter frequency, every effect finite and not silencing, silence stays silent, pitch detection, autotune snapping to the semitone.
- `hotkeys`: normalization, name round trip, invalid keys.
- `i18n`: same keys and fields in both languages.
- `library` (ported from `tests/test_library.py`): group order, moving only within the collection, edges, identical empty slots (by id), changing collection goes to the end, rename, delete keeps the sounds, move/collapse collection, name validation, old configs and duplicate/blank collections.
- `config` (ported from `tests/test_config.py`): fresh install, import from the old version (config, sounds and clip; nothing deleted), replaced program folder keeps everything, old config copied again doesn't win, import doesn't overwrite sounds, corrupted config kept aside, round trip preserving unknown fields.
- `cable_roundtrip` (ignored by default): plays a quiet 1 kHz tone into `CABLE Input`, records it through the loopback of the same device and checks frequency and amplitude in the saved WAV. Nothing comes out of the speakers.

### Environment variables (diagnostics)
- `SOUNDMOD_DATA_DIR`: uses another data folder instead of `%APPDATA%\SoundMod` (tests, screenshots without touching real data).
- `SOUNDMOD_DEBUG_KEYS=1`: prints to stderr every key the hook receives and every combo built (run the debug build from a terminal).

**Simulated keys in tests:** processes started inside an isolated environment (an automation tool's sandbox) may not receive in the hook the keys injected with `keybd_event`/`SendInput`. Started normally (Explorer, shortcut, the user's terminal) the hook receives them; to test global keys automatically, start the app outside the sandbox.

## 7. Status and open items

Verified on the development machine: opening mic/cable/headphones and loopback, `cable_roundtrip` (1000 Hz, peak 0.100), global F1 and F5 through the hook (played the slot; saved the clip), interface in pt and en, icon and version in the `.exe`. Order and collections through the interface: ▲▼, collection menu, new collection (duplicate name refused with a message; valid name created), collapse, all saved in `config.json`; import of the old config with the status-bar notice. The 8 colors and the Sunset theme, applied live.

Not verified yet:
1. Voice effects with a real voice, by ear (only synthetic signals in tests).
2. Key capture through the interface (✎) with a real keyboard.
3. Automatic language detection on an English Windows.

Ideas: system tray, start with Windows, drag and drop files, fade-out on stop, adjustable effect intensity, renaming saved sounds.
