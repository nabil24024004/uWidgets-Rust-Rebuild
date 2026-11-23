# Domain Glossary (`uWidgets` C# -> Rust)

| Original C# Term | Meaning | Rust Crate / Symbol |
| --- | --- | --- |
| `AppSettings` | Root configuration in `appSettings.json` | `uwidgets_core::models::AppSettings` |
| `Theme` | Appearance preset/config (DarkMode, AccentColor, OpacityLevel, Monochrome, UseNativeFrame, FontFamily) | `uwidgets_core::models::Theme` |
| `Layout` | SnapSize, LockSize, SnapPosition, LockPosition flags | `uwidgets_core::models::LayoutConfig` |
| `Dimensions` | Base unit `Size` (72), `Margin` (16), `Radius` (24) | `uwidgets_core::models::Dimensions` |
| `WidgetLayout` | Single widget entry in `layout.json` | `uwidgets_core::models::WidgetLayout` |
| `GridService` | Cell-to-pixel sizing and snap math | `uwidgets_core::port` / `uwidgets_core::grid` |
| `MonthCalendarModel` | Calendar `Month` widget settings (`FirstDayOfWeek`) | `widget_calendar::MonthCalendarModel` |
| `ClockModel` / `WorldClockModel` | Analog/Digital/World clock settings | `widget_clock::ClockModel`, `WorldClockModel` |
| `ClockHandViewModel` | Hand rotation angle, length, and SVG path | `widget_clock::ClockHand` |
| `SingleMetricModel` / `MetricType` | Monitor widget metric selector (0..4) | `widget_monitor::MetricType` |
| `StrokeDashOffset` | Circular arc progress offset (`31.4*(1-v)` for Monitor, `21.0*norm` for Weather) | `widget_monitor::stroke_dash_offset`, `widget_weather::metric_stroke_dash_offset` |
| `NoteModel` | Sticky note Title, Content, Updated | `widget_notes_reminders::NoteModel` |
| `RemindersListModel` / `ReminderModel` | Checklist name & items (`Completed`, `Title`) | `widget_notes_reminders::RemindersListModel` |
| `ForecastModel` | Weather city, lat, lon, temperature unit | `widget_weather::ForecastModel` |
| `WeatherCode` | WMO weather code (0..99, 28 variants) | `widget_weather::WeatherCode` |
| `WeatherIcon` | 15 vector weather/sun/moon/pressure/wind icons | `widget_weather::WeatherIconKind` |
| `ClickThroughTextBox` | Single-click drags widget, double-click edits text | `app::ui::inline_edit::InlineTextField` |
