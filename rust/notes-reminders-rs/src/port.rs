//! The Rust port of `NotesReminders/`.
//!
//! Faithful port of `NotesReminders/NotesRemindersLogic.cs` verified against `.NET` via `rust-anything`.

#![forbid(unsafe_code)]

/// Port of `RemindersLayoutTier` (`NotesReminders/NotesRemindersLogic.cs:5`).
pub fn reminders_layout_tier(width: f64, height: f64) -> Result<String, String> {
    if width < 200.0 && height < 200.0 {
        return Ok("Small".to_string());
    }
    let wide = height > 0.0 && (width / height) >= 1.5;
    Ok(if wide { "Wide" } else { "Large" }.to_string())
}

/// Port of `ShouldRemoveReminderOnEdit` (`NotesReminders/NotesRemindersLogic.cs:16`).
pub fn should_remove_reminder_on_edit(new_text: String) -> Result<bool, String> {
    Ok(new_text.is_empty())
}

/// Port of `ShouldAddReminder` (`NotesReminders/NotesRemindersLogic.cs:21`).
pub fn should_add_reminder(input_text: String) -> Result<bool, String> {
    Ok(!input_text.is_empty())
}

/// Port of `CountRemaining` (`NotesReminders/NotesRemindersLogic.cs:26`).
pub fn count_remaining(completed_flags: Vec<bool>) -> Result<i64, String> {
    let count = completed_flags.iter().filter(|&&done| !done).count() as i32;
    Ok(i64::from(count))
}

/// Port of `CountTotal` (`NotesReminders/NotesRemindersLogic.cs:39`).
pub fn count_total(completed_flags: Vec<bool>) -> Result<i64, String> {
    Ok(i64::from(completed_flags.len() as i32))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notes_reminders_parity() {
        assert_eq!(
            reminders_layout_tier(160.0, 160.0).ok().as_deref(),
            Some("Small")
        );
        assert_eq!(
            reminders_layout_tier(336.0, 160.0).ok().as_deref(),
            Some("Wide")
        );
        assert_eq!(
            reminders_layout_tier(336.0, 336.0).ok().as_deref(),
            Some("Large")
        );
        assert_eq!(count_remaining(vec![true, false, false]).ok(), Some(2));
        assert_eq!(count_total(vec![true, false, false]).ok(), Some(3));
    }
}
