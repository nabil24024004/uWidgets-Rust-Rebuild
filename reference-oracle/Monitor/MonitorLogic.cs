using System;
using System.Globalization;

namespace UWidgets.Oracle.Monitor;

public static class MonitorLogic
{
    public static double MonitorStrokeDashOffset(double value)
    {
        return 31.4 * (1.0 - value);
    }

    public static string MonitorPercentText(double value)
    {
        int pct = (int)Math.Round(value * 100.0);
        return $"{pct}%";
    }

    public static bool MonitorIsCompact(double height)
    {
        return height < 100.0;
    }

    public static int MonitorMargin(double height)
    {
        return height < 100.0 ? 6 : 12;
    }

    public static double ComputeRamRatio(double totalKb, double freeKb)
    {
        if (totalKb <= 0.0)
        {
            return 0.0;
        }
        return (totalKb - freeKb) / totalKb;
    }

    public static double ComputeNetworkRatio(double sentBytesPerSec, double recvBytesPerSec, double linkSpeedBitsPerSec)
    {
        if (linkSpeedBitsPerSec <= 0.0)
        {
            return 0.0;
        }
        return (sentBytesPerSec + recvBytesPerSec) * 8.0 / linkSpeedBitsPerSec;
    }

    public static string MetricLocaleKey(int metricType)
    {
        return metricType switch
        {
            0 => "Monitor_Metric_0",
            1 => "Monitor_Metric_1",
            2 => "Monitor_Metric_2",
            3 => "Monitor_Metric_3",
            4 => "Monitor_Metric_4",
            _ => "Monitor_Metric_0",
        };
    }
}
