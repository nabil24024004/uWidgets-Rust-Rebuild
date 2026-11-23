using System;

namespace UWidgets.Oracle.Core;

public static class CoreLogic
{
    public static int GridGetSize(int units, int size, int margin, double scaling, bool addMargin)
    {
        if (addMargin)
        {
            return (int)(scaling * (double)units * (double)(size + margin) + scaling * (double)margin);
        }
        return (int)(scaling * (double)units * (double)(size + margin) - scaling * (double)margin);
    }

    public static int GridSnapDimension(double pixels, int size, int margin, double scaling, bool addMargin, int minValue)
    {
        double step = scaling * (double)(size + margin);
        if (step <= 0.0)
        {
            return 0;
        }
        int units = (int)Math.Max((double)minValue, Math.Round(pixels / step));
        return GridGetSize(units, size, margin, scaling, addMargin);
    }

    public static int GridSnapSize(double pixels, int size, int margin)
    {
        return GridSnapDimension(pixels, size, margin, 1.0, false, 1);
    }

    public static int GridSnapPosition(double pixels, int size, int margin, double scaling)
    {
        return GridSnapDimension(pixels, size, margin, scaling, true, 0);
    }

    public static double ContentScale(int dimensionSize)
    {
        return (double)dimensionSize / 72.0;
    }

    public static int GalleryDefaultDimension(int size, int margin)
    {
        return 2 * size + margin;
    }

    public static int PresetColumns(int presetIndex)
    {
        return presetIndex switch
        {
            0 => 2,
            1 => 4,
            2 => 4,
            3 => 8,
            _ => 2,
        };
    }

    public static int PresetRows(int presetIndex)
    {
        return presetIndex switch
        {
            0 => 2,
            1 => 2,
            2 => 4,
            3 => 4,
            _ => 2,
        };
    }

    public static double ThemeTextOpacity(double opacityLevel)
    {
        return opacityLevel < 1.0 ? 0.8 : 1.0;
    }

    public static bool ThemeUsesAcrylic(double opacityLevel)
    {
        return opacityLevel < 1.0;
    }

    public static int ThemePreviewRadius(bool useNativeFrame)
    {
        return useNativeFrame ? 2 : 10;
    }

    public static int ThemePreviewBorder(bool useNativeFrame)
    {
        return useNativeFrame ? 1 : 0;
    }
}
