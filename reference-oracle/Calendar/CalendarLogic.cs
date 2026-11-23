using System;

namespace UWidgets.Oracle.Calendar;

public static class CalendarLogic
{
    public static int DaysInMonth(int year, int month)
    {
        if (year < 1 || year > 9999 || month < 1 || month > 12)
        {
            return 0;
        }
        return DateTime.DaysInMonth(year, month);
    }

    public static int EmptyDaysCount(int firstOfMonthDayOfWeek, int firstDayOfWeek)
    {
        int dow = ((firstOfMonthDayOfWeek % 7) + 7) % 7;
        int first = ((firstDayOfWeek % 7) + 7) % 7;
        return (dow + first + 5) % 7;
    }

    public static bool IsWeekend(int dayOfWeek)
    {
        int d = ((dayOfWeek % 7) + 7) % 7;
        return d == 0 || d == 6;
    }

    public static int[] WeekdayOrder(int firstDayOfWeek)
    {
        int offset = ((firstDayOfWeek % 7) + 7) % 7;
        int[] result = new int[7];
        for (int i = 0; i < 7; i++)
        {
            result[i] = (i + offset) % 7;
        }
        return result;
    }

    public static double MonthTextSize(double width, double height)
    {
        return Math.Min(width, height) / 12.0;
    }

    public static double MonthMarginLeft(double width, double height)
    {
        return 0.0 - Math.Min(width, height) / 30.0;
    }

    public static string StripTrailingDot(string input)
    {
        return input.Replace(".", "");
    }
}
