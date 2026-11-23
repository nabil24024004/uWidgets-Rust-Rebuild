# Rust migration plan: reference-oracle (C#)

1 package, 6 source files (436 lines), 0 test files, 0 UI files, grouped into 6 units.

| Ready | Caution | Blocked | Keep in source language |
| ---: | ---: | ---: | ---: |
| 6 | 0 | 0 | 0 |

## Start here

Best first ports: safe to port (readiness) and worth porting (loops and string work, tests, size).

| # | Unit | Lines | Priority | Readiness | Tests | Bridge | Main risks |
| ---: | --- | ---: | ---: | ---: | ---: | --- | --- |
| 1 | `NotesReminders/` | 38 | 58 | 97 | 0 | P/Invoke (C ABI) | Division and modulo (1) |
| 2 | `Calendar/` | 46 | 56 | 91 | 0 | P/Invoke (C ABI) | Division and modulo (12) |
| 3 | `Weather/` | 117 | 54 | 87 | 0 | P/Invoke (C ABI) | Dynamic typing (2); Integer width and overflow (2); Division and modulo (2) |
| 4 | `Monitor/` | 51 | 51 | 89 | 0 | P/Invoke (C ABI) | Integer width and overflow (3); Division and modulo (2); Dynamic typing (1) |
| 5 | `Clock/` | 107 | 51 | 74 | 0 | P/Invoke (C ABI) | String offsets and slicing (2); Division and modulo (9); Dynamic typing (6) |
| 6 | `Core/` | 77 | 50 | 80 | 0 | P/Invoke (C ABI) | Dynamic typing (12); Integer width and overflow (8); Division and modulo (2) |

### `NotesReminders/`

38 lines in 1 file, wave 0, priority 58, readiness 97, bridge **P/Invoke (C ABI)**.

- Leaf: imports no other unit.
- No tests found: write characterization tests against the original code first.
- Division and modulo x1: `NotesReminders/NotesRemindersLogic.cs:12`

### `Calendar/`

46 lines in 1 file, wave 0, priority 56, readiness 91, bridge **P/Invoke (C ABI)**.

- Leaf: imports no other unit.
- No tests found: write characterization tests against the original code first.
- Division and modulo x12: `Calendar/CalendarLogic.cs:18`, `Calendar/CalendarLogic.cs:18`, `Calendar/CalendarLogic.cs:19`

### `Weather/`

117 lines in 1 file, wave 0, priority 54, readiness 87, bridge **P/Invoke (C ABI)**.

- Leaf: imports no other unit.
- No tests found: write characterization tests against the original code first.
- Dynamic typing x2: `Weather/WeatherLogic.cs:53`, `Weather/WeatherLogic.cs:58`
- Integer width and overflow x2: `Weather/WeatherLogic.cs:75`, `Weather/WeatherLogic.cs:91`
- Division and modulo x2: `Weather/WeatherLogic.cs:64`, `Weather/WeatherLogic.cs:75`

### `Monitor/`

51 lines in 1 file, wave 0, priority 51, readiness 89, bridge **P/Invoke (C ABI)**.

- Leaf: imports no other unit.
- No tests found: write characterization tests against the original code first.
- Integer width and overflow x3: `Monitor/MonitorLogic.cs:10`, `Monitor/MonitorLogic.cs:15`, `Monitor/MonitorLogic.cs:44`
- Division and modulo x2: `Monitor/MonitorLogic.cs:35`, `Monitor/MonitorLogic.cs:44`
- Dynamic typing x1: `Monitor/MonitorLogic.cs:15`

### `Clock/`

107 lines in 1 file, wave 0, priority 51, readiness 74, bridge **P/Invoke (C ABI)**.

- Leaf: imports no other unit.
- No tests found: write characterization tests against the original code first.
- String offsets and slicing x2: `Clock/ClockLogic.cs:56`, `Clock/ClockLogic.cs:71`
- Division and modulo x9: `Clock/ClockLogic.cs:9`, `Clock/ClockLogic.cs:14`, `Clock/ClockLogic.cs:19`
- Dynamic typing x6: `Clock/ClockLogic.cs:9`, `Clock/ClockLogic.cs:9`, `Clock/ClockLogic.cs:14`
- Integer width and overflow x3: `Clock/ClockLogic.cs:9`, `Clock/ClockLogic.cs:14`, `Clock/ClockLogic.cs:19`

## Order by package

Packages come after the workspace packages they use. Inside a package, port by wave: a unit's imports are in earlier waves.

### 1. ReferenceOracle (`.`, portable, 0 test files)

| Wave | Unit | Status | Lines | Readiness | Bridge | Main risks |
| ---: | --- | --- | ---: | ---: | --- | --- |
| 0 | `NotesReminders/` | ready | 38 | 97 | P/Invoke (C ABI) | Division and modulo (1) |
| 0 | `Calendar/` | ready | 46 | 91 | P/Invoke (C ABI) | Division and modulo (12) |
| 0 | `Monitor/` | ready | 51 | 89 | P/Invoke (C ABI) | Integer width and overflow (3); Division and modulo (2) |
| 0 | `Weather/` | ready | 117 | 87 | P/Invoke (C ABI) | Dynamic typing (2); Integer width and overflow (2) |
| 0 | `Core/` | ready | 77 | 80 | P/Invoke (C ABI) | Dynamic typing (12); Integer width and overflow (8) |
| 0 | `Clock/` | ready | 107 | 74 | P/Invoke (C ABI) | String offsets and slicing (2); Division and modulo (9) |

## Risk guide

- **String offsets and slicing** (High): JS indexes strings in UTF-16 code units, Rust in UTF-8 bytes. Offsets that cross the boundary must be converted, or non-ASCII text gets wrong ranges silently.
- **Dynamic typing** (Medium): Values whose shape is checked at runtime need an explicit Rust enum or `serde_json::Value`.
- **Integer width and overflow** (Medium): Python ints never overflow; C, Go and Java ints wrap or are undefined on overflow. Pick i64, i128 or a bignum crate deliberately, and use wrapping_* where the source wraps.
- **Division and modulo** (Medium): Python `//` and `%` floor toward negative infinity; Rust `/` and `%` truncate: use `div_euclid`/`rem_euclid` or explicit floor math. C division by zero is undefined.
