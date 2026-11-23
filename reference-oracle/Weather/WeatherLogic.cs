using System;
using System.Globalization;

namespace UWidgets.Oracle.Weather;

public static class WeatherLogic
{
    public static string WeatherIconName(int weatherCode, bool isDay)
    {
        if (!isDay)
        {
            return "Night";
        }
        return weatherCode switch
        {
            0 or 1 => "Clear",
            2 => "PartlyClear",
            3 => "Cloudy",
            45 or 48 => "Fog",
            51 or 53 or 55 or 56 or 57 => "Drizzle",
            61 or 63 or 66 => "Rain",
            65 or 67 or 80 or 81 or 82 => "HeavyRain",
            71 or 73 => "Snow",
            75 or 77 or 85 or 86 => "HeavySnow",
            95 or 96 or 99 => "Thunderstorm",
            _ => "None",
        };
    }

    public static string ForecastLayoutTier(double width, double height)
    {
        if (width > 230.0)
        {
            if (height > 230.0)
            {
                return "Large";
            }
            if (height > 150.0)
            {
                return "Wide";
            }
            return "Tiny";
        }
        if (width > 150.0 && height > 150.0)
        {
            return "Small";
        }
        return "Tiny";
    }

    public static int MetricDisplayValue(double value)
    {
        return (int)Math.Round(value);
    }

    public static int MetricFontSize(double value)
    {
        int rounded = (int)Math.Round(value);
        string text = rounded.ToString(CultureInfo.InvariantCulture);
        if (text.Length <= 2)
        {
            return 50;
        }
        return 100 / text.Length;
    }

    public static double MetricStrokeDashOffset(double min, double max, double value)
    {
        double span = max - min;
        if (span == 0.0)
        {
            return 0.0;
        }
        double clamped = Math.Clamp(value, min, max);
        return (clamped - min) / span * 21.0;
    }

    public static double[] DailyGraphWeights(double dayMin, double dayMax, double globalMin, double globalMax)
    {
        return new double[]
        {
            dayMin - globalMin,
            dayMax - dayMin,
            globalMax - dayMax,
        };
    }

    public static double MetricMargin(double width, double height)
    {
        double m = Math.Min(width, height);
        return m >= 150.0 ? m * 0.1 : 6.0;
    }

    public static string NextSunEventKind(long nowUnix, long todaySunriseUnix, long todaySunsetUnix)
    {
        if (nowUnix < todaySunriseUnix)
        {
            return "SunriseToday";
        }
        if (nowUnix < todaySunsetUnix)
        {
            return "SunsetToday";
        }
        return "SunriseTomorrow";
    }

    public static string BuildForecastUrl(double latitude, double longitude, string temperatureUnit)
    {
        string lat = string.Format(CultureInfo.InvariantCulture, "{0:F4}", latitude);
        string lon = string.Format(CultureInfo.InvariantCulture, "{0:F4}", longitude);
        return $"https://api.open-meteo.com/v1/forecast?latitude={lat}&longitude={lon}&temperature_unit={temperatureUnit}&current=temperature_2m,weathercode,surface_pressure&hourly=temperature_2m,weathercode,uv_index,is_day&daily=temperature_2m_min,temperature_2m_max,weathercode,sunrise,sunset&timezone=auto";
    }

    public static string BuildAirQualityUrl(double latitude, double longitude)
    {
        string lat = string.Format(CultureInfo.InvariantCulture, "{0:F4}", latitude);
        string lon = string.Format(CultureInfo.InvariantCulture, "{0:F4}", longitude);
        return $"https://air-quality-api.open-meteo.com/v1/air-quality?latitude={lat}&longitude={lon}&current=european_aqi";
    }

    public static string BuildGeocodingUrl(string cityName, string twoLetterLang)
    {
        return $"https://geocoding-api.open-meteo.com/v1/search?name={cityName}&language={twoLetterLang}";
    }

    public static string FormatTemperature(double temp)
    {
        return string.Format(CultureInfo.InvariantCulture, "{0:0}\u00B0", temp);
    }
}
