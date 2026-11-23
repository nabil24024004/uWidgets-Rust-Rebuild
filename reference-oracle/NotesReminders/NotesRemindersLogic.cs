namespace UWidgets.Oracle.NotesReminders;

public static class NotesRemindersLogic
{
    public static string RemindersLayoutTier(double width, double height)
    {
        bool small = width < 200.0 && height < 200.0;
        if (small)
        {
            return "Small";
        }
        bool wide = height > 0.0 && (width / height) >= 1.5;
        return wide ? "Wide" : "Large";
    }

    public static bool ShouldRemoveReminderOnEdit(string newText)
    {
        return string.IsNullOrEmpty(newText);
    }

    public static bool ShouldAddReminder(string inputText)
    {
        return !string.IsNullOrEmpty(inputText);
    }

    public static int CountRemaining(bool[] completedFlags)
    {
        int remaining = 0;
        for (int i = 0; i < completedFlags.Length; i++)
        {
            if (!completedFlags[i])
            {
                remaining++;
            }
        }
        return remaining;
    }

    public static int CountTotal(bool[] completedFlags)
    {
        return completedFlags.Length;
    }
}
