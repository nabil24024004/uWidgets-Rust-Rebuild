using System;

namespace UWidgets.Oracle.Clock;

public static class ClockLogic
{
    public static double SecondsAngle(int second, int millisecond)
    {
        return ((double)second + (double)millisecond / 1000.0) * 6.0;
    }

    public static double MinutesAngle(int minute, int second)
    {
        return ((double)minute + (double)second / 60.0) * 6.0;
    }

    public static double HoursAngle(int hour, int minute)
    {
        return ((double)(hour % 12) + (double)minute / 60.0) * 30.0;
    }

    public static int ClockHandEndY(int length)
    {
        return 420 - length;
    }

    public static string ClockHandPath(int length)
    {
        return $"M 500,420 V {420 - length}";
    }

    public static bool WorldIsWideLayout(double width, double height)
    {
        if (height <= 0.0)
        {
            return false;
        }
        return (width / height) >= 1.5;
    }

    public static string CityFullName(string displayName)
    {
        string[] parts = displayName.Split(") ");
        string afterOffset = parts[parts.Length - 1];
        string firstComma = afterOffset.Split(", ")[0];
        return firstComma.Split("(")[0];
    }

    public static string CityShortName(string displayName)
    {
        string full = CityFullName(displayName);
        if (full.Length <= 11)
        {
            return full;
        }
        return full.Substring(0, 9) + "\u2026";
    }

    public static string CityAbbreviation(string displayName, bool isLocal)
    {
        if (isLocal)
        {
            return "";
        }
        string[] parts = displayName.Split(") ");
        string tail = parts[parts.Length - 1];
        if (tail.Length < 3)
        {
            return tail.ToUpperInvariant();
        }
        return tail.Substring(0, 3).ToUpperInvariant();
    }

    public static string FormatTimeZoneDiff(int diffMinutes)
    {
        int hours = diffMinutes / 60;
        TimeSpan span = TimeSpan.FromMinutes(Math.Abs(diffMinutes));
        if (hours > 0)
        {
            return $"+{span:hh\\:mm}";
        }
        if (hours < 0)
        {
            return $"-{span:hh\\:mm}";
        }
        return "00:00";
    }

    public static string RelativeDayKey(int dayDelta)
    {
        if (dayDelta < 0)
        {
            return "Clock_Yesterday";
        }
        if (dayDelta > 0)
        {
            return "Clock_Tomorrow";
        }
        return "Clock_Today";
    }

    public static string FormatDigitalTime(int hour24, int minute, int second, bool use24Hours, bool showSeconds)
    {
        int h = ((hour24 % 24) + 24) % 24;
        int m = Math.Clamp(minute, 0, 59);
        int s = Math.Clamp(second, 0, 59);
        string sec = showSeconds ? $":{s:D2}" : "";
        if (use24Hours)
        {
            return $"{h:D2}:{m:D2}{sec}";
        }
        int h12 = h % 12;
        if (h12 == 0)
        {
            h12 = 12;
        }
        string ampm = h < 12 ? "AM" : "PM";
        return $"{h12:D2}:{m:D2}{sec} {ampm}";
    }
}
