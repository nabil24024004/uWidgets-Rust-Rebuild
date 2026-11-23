# Whole-App Port Map: `uWidgets` -> Pure Rust (`uwidgets-rs`)

- **Goal**: Standalone pure Rust Windows desktop widget application matching every exact feature, config schema, visual style, and behavior of `C:\uWidgets` (v0.6.0.0) at a fraction of the RAM (~10–20 MB vs 190 MB) and disk size (~3–6 MB vs 60 MB).
- **Governance Docs**: [DECISIONS.md](DECISIONS.md) · [GLOSSARY.md](GLOSSARY.md) · [research/host-and-core.md](research/host-and-core.md) · [research/widgets-spec.md](research/widgets-spec.md) · [research/locales-and-assets.md](research/locales-and-assets.md)

## Wave Plan & Unit Status

| Wave | Unit / Crate | Reference Oracle Unit | Status | Parity (`rust-anything diff` / `gate`) |
| --- | --- | --- | --- | --- |
| 1 | `rust/core-rs` | `Core/CoreLogic.cs` | `done` | **945/945** · Gate PASSED |
| 2 | `rust/calendar-rs` | `Calendar/CalendarLogic.cs` | `done` | **912/912** · Gate PASSED |
| 2 | `rust/clock-rs` | `Clock/ClockLogic.cs` | `done` | **1,424/1,424** (12 UTF-16 lone-surrogate known-diffs) · Gate PASSED |
| 3 | `rust/weather-rs` | `Weather/WeatherLogic.cs` | `done` | **1,899/1,899** · Gate PASSED |
| 3 | `rust/monitor-rs` | `Monitor/MonitorLogic.cs` | `done` | **552/552** · Gate PASSED |
| 3 | `rust/notes-reminders-rs` | `NotesReminders/NotesRemindersLogic.cs` | `done` | **668/668** · Gate PASSED |
| 4 | `rust/app` (`uwidgets-rs.exe`) | Native Win32 + Direct2D/DirectWrite GUI Shell | `done` | **3.22 MB** exe · **18.69 MB** Private RAM · **218 ms** startup · `clippy`/`test` PASSED |

**Total Gated Parity Coverage**: **6,400 / 6,400** recorded `.NET` cases verified across 6 `rust-anything` crates + 7/7 native app integration tests.

## Out of Scope
- Embedding a .NET CLR to execute third-party `.NET` `.dll` widget plugins (ADR-002).
- Automatic prompts to download upstream C# releases from `github.com/creewick/uWidgets` (ADR-005).
