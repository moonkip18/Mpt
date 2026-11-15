import 'package:flutter/material.dart';

/// Palette lifted from `indexstudy.html`: near-black OLED ground, hairline
/// rules, one muted red accent, cool off-white ink.
class MptColors {
  static const background = Color(0xFF050506);
  static const surface = Color(0xFF0A0B0D);
  static const surfaceRaised = Color(0xFF0D0F12);
  static const ink = Color(0xFFE9EAEC);
  static const inkDim = Color(0xFF8B8F97);
  static const inkFaint = Color(0xFF565A61);
  static const line = Color(0x14FFFFFF);
  static const lineStrong = Color(0x24FFFFFF);
  static const synapse = Color(0xFFDFE3E8);
  static const accent = Color(0xFFC8433C);
  static const accentSoft = Color(0x66C8433C);
  static const ok = Color(0xFF6E8F72);
}

/// System faces only: no font downloads, instant first frame on Windows.
class MptFonts {
  static const ui = 'Segoe UI';
  static const display = 'Georgia';
  static const mono = 'Consolas';
}

class MptText {
  static const hero = TextStyle(
    fontFamily: MptFonts.display,
    fontSize: 96,
    height: 1.0,
    fontWeight: FontWeight.w400,
    letterSpacing: -2,
    color: MptColors.ink,
  );

  static const heroSmall = TextStyle(
    fontFamily: MptFonts.display,
    fontSize: 42,
    height: 1.0,
    fontWeight: FontWeight.w400,
    letterSpacing: -1,
    color: MptColors.ink,
  );

  static const section = TextStyle(
    fontFamily: MptFonts.ui,
    fontSize: 11,
    height: 1.2,
    fontWeight: FontWeight.w600,
    letterSpacing: 1.6,
    color: MptColors.inkFaint,
  );

  static const body = TextStyle(
    fontFamily: MptFonts.ui,
    fontSize: 13,
    height: 1.35,
    fontWeight: FontWeight.w400,
    color: MptColors.ink,
  );

  static const bodyDim = TextStyle(
    fontFamily: MptFonts.ui,
    fontSize: 12,
    height: 1.35,
    fontWeight: FontWeight.w400,
    color: MptColors.inkDim,
  );

  static const label = TextStyle(
    fontFamily: MptFonts.mono,
    fontSize: 11,
    height: 1.2,
    fontWeight: FontWeight.w400,
    letterSpacing: 0.4,
    color: MptColors.inkDim,
  );

  static const numeric = TextStyle(
    fontFamily: MptFonts.mono,
    fontSize: 13,
    height: 1.2,
    fontWeight: FontWeight.w500,
    color: MptColors.ink,
  );

  static const numericLarge = TextStyle(
    fontFamily: MptFonts.mono,
    fontSize: 22,
    height: 1.1,
    fontWeight: FontWeight.w400,
    letterSpacing: -0.5,
    color: MptColors.ink,
  );
}

class MptTheme {
  static ThemeData build() {
    final base = ThemeData.dark(useMaterial3: true);
    return base.copyWith(
      scaffoldBackgroundColor: MptColors.background,
      canvasColor: MptColors.background,
      colorScheme: const ColorScheme.dark(
        surface: MptColors.background,
        primary: MptColors.synapse,
        secondary: MptColors.accent,
        error: MptColors.accent,
        onSurface: MptColors.ink,
        onPrimary: MptColors.background,
      ),
      dividerColor: MptColors.line,
      dividerTheme: const DividerThemeData(
        color: MptColors.line,
        thickness: 1,
        space: 1,
      ),
      textTheme: base.textTheme.apply(
        fontFamily: MptFonts.ui,
        bodyColor: MptColors.ink,
        displayColor: MptColors.ink,
      ),
      tooltipTheme: TooltipThemeData(
        decoration: BoxDecoration(
          color: MptColors.surfaceRaised,
          border: Border.all(color: MptColors.lineStrong),
          borderRadius: BorderRadius.circular(4),
        ),
        textStyle: MptText.bodyDim,
        waitDuration: const Duration(milliseconds: 500),
      ),
      scrollbarTheme: ScrollbarThemeData(
        thickness: WidgetStateProperty.all(6),
        thumbColor: WidgetStateProperty.all(const Color(0xFF242933)),
        radius: const Radius.circular(3),
      ),
      splashFactory: NoSplash.splashFactory,
      highlightColor: Colors.transparent,
    );
  }
}
