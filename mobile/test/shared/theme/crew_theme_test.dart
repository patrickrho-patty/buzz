import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:crew/shared/theme/theme.dart';
import 'package:crew/shared/widgets/frosted_app_bar.dart';

void main() {
  group('Crew theme catalog entries', () {
    test('both halves are in the catalog', () {
      expect(findTheme(crewThemeName), isNotNull);
      expect(findTheme(crewDarkThemeName), isNotNull);
    });

    test('borrow the GitHub palettes', () {
      final crew = findTheme(crewThemeName)!;
      final github = findTheme('github-light')!;
      expect(crew.bg, github.bg);
      expect(crew.fg, github.fg);
      expect(crew.comment, github.comment);

      final crewDark = findTheme(crewDarkThemeName)!;
      final githubDark = findTheme('github-dark')!;
      expect(crewDark.bg, githubDark.bg);
      expect(crewDark.fg, githubDark.fg);
      expect(crewDark.comment, githubDark.comment);
    });

    test('are a light/dark pair', () {
      expect(findTheme(crewThemeName)!.isDark, isFalse);
      expect(findTheme(crewDarkThemeName)!.isDark, isTrue);
      expect(themePairFor(crewThemeName), crewDarkThemeName);
      expect(themePairFor(crewDarkThemeName), crewThemeName);
    });

    test('appear as a single System-mode option labelled "Crew"', () {
      final paired = themeGroups().paired.map((t) => t.name);
      expect(paired, contains(crewThemeName));
      expect(paired, isNot(contains(crewDarkThemeName)));
      expect(pairedThemeLabel(crewThemeName), 'Crew');
      expect(themeSelectionLabel(crewThemeName, ThemeMode.system), 'Crew');
      expect(themeSelectionLabel(crewDarkThemeName, ThemeMode.system), 'Crew');
    });

    test('forces neutral rendering without changing the stored accent', () {
      const storedAccent = '#ef4444';

      expect(
        effectiveAccentIndex(crewThemeName, storedAccent),
        neutralAccentIndex,
      );
      expect(
        effectiveAccentIndex(crewDarkThemeName, storedAccent),
        neutralAccentIndex,
      );
      expect(
        effectiveAccentIndex('github-light', storedAccent),
        accentIndexForWireValue(storedAccent),
      );
      expect(storedAccent, '#ef4444');
    });

    test('resolve across brightnesses like any other pair', () {
      final resolved = resolveSchemes(crewThemeName, ThemeMode.system);
      expect(resolved.forcedMode, isNull);
      expect(resolved.light.brightness, Brightness.light);
      expect(resolved.dark.brightness, Brightness.dark);
      expect(resolved.lightTheme?.name, crewThemeName);
      expect(resolved.darkTheme?.name, crewDarkThemeName);

      expect(
        effectiveTheme(crewThemeName, ThemeMode.dark)?.name,
        crewDarkThemeName,
      );
      expect(
        effectiveTheme(crewDarkThemeName, ThemeMode.light)?.name,
        crewThemeName,
      );
    });

    test(
      'fallbacks expose the effective Crew theme for gradient selection',
      () {
        final coerced = resolveSchemes('nord', ThemeMode.light);
        expect(coerced.lightTheme?.name, crewThemeName);
        expect(
          crewTopSectionGradient(
            coerced.lightTheme!.name,
            coerced.light.brightness,
          ),
          isNotNull,
        );

        final unknown = resolveSchemes('not-a-theme', ThemeMode.light);
        expect(unknown.lightTheme?.name, crewThemeName);
        expect(
          crewTopSectionGradient(
            unknown.lightTheme!.name,
            unknown.light.brightness,
          ),
          isNotNull,
        );
      },
    );
  });

  group('crewTopSectionGradient', () {
    test('is null for non-Crew themes', () {
      expect(crewTopSectionGradient('github-light', Brightness.light), isNull);
      expect(crewTopSectionGradient('nord', Brightness.dark), isNull);
    });

    test('paints top to bottom for both halves of the pair', () {
      for (final name in [crewThemeName, crewDarkThemeName]) {
        final gradient = crewTopSectionGradient(name, Brightness.light);
        expect(gradient, isNotNull, reason: '$name should be gradient-backed');
        expect(gradient!.begin, Alignment.topCenter);
        expect(gradient.end, Alignment.bottomCenter);
        expect(gradient.colors, hasLength(2));
      }
    });

    test('brightness selects the stops, not the theme name', () {
      // Both halves enable the gradient, so System mode keeps it on across an
      // OS switch — the applied brightness alone decides which stops are used.
      final light = crewTopSectionGradient(crewThemeName, Brightness.light)!;
      final dark = crewTopSectionGradient(crewThemeName, Brightness.dark)!;

      expect(light.colors, isNot(dark.colors));
      expect(
        crewTopSectionGradient(crewDarkThemeName, Brightness.dark)!.colors,
        dark.colors,
      );
      expect(
        crewTopSectionGradient(crewDarkThemeName, Brightness.light)!.colors,
        light.colors,
      );
    });

    test('is opaque so the color replaces the frosted fill', () {
      for (final brightness in Brightness.values) {
        final gradient = crewTopSectionGradient(crewThemeName, brightness)!;
        for (final color in gradient.colors) {
          expect(color.a, 1.0);
        }
      }
    });
  });

  group('theme threading', () {
    BoxDecoration barDecoration(WidgetTester tester) {
      final container = tester
          .widgetList<Container>(
            find.descendant(
              of: find.byType(FrostedAppBar),
              matching: find.byType(Container),
            ),
          )
          .first;
      return container.decoration! as BoxDecoration;
    }

    Widget harness(ThemeData theme) => MaterialApp(
      theme: theme,
      home: Builder(
        builder: (context) => Stack(
          children: [
            FrostedAppBar(
              gradient: context.appColors.topSectionGradient,
              title: const Text('Home'),
            ),
          ],
        ),
      ),
    );

    testWidgets('AppTheme carries the gradient to the top section', (
      tester,
    ) async {
      await tester.pumpWidget(
        harness(
          AppTheme.light(
            topSectionGradient: crewTopSectionGradient(
              crewThemeName,
              Brightness.light,
            ),
          ),
        ),
      );

      final decoration = barDecoration(tester);
      expect(decoration.gradient, isNotNull);
      // A BoxDecoration cannot paint a color and a gradient at once.
      expect(decoration.color, isNull);
    });

    testWidgets('non-Crew themes keep the frosted surface fill', (
      tester,
    ) async {
      await tester.pumpWidget(harness(AppTheme.light()));

      final decoration = barDecoration(tester);
      expect(decoration.gradient, isNull);
      expect(decoration.color, isNotNull);
    });

    testWidgets('Crew section labels use 80% neutral foreground', (
      tester,
    ) async {
      await tester.pumpWidget(
        harness(
          AppTheme.light(
            topSectionGradient: crewTopSectionGradient(
              crewThemeName,
              Brightness.light,
            ),
          ),
        ),
      );

      final context = tester.element(find.text('Home'));
      expect(
        navigationSectionForeground(context),
        Colors.black.withValues(alpha: 0.8),
      );
    });

    testWidgets('navigation roles inherit non-Crew theme tokens', (
      tester,
    ) async {
      const primaryForeground = Color(0xFF123456);
      const secondaryForeground = Color(0xFF789ABC);
      const searchSurface = Color(0xFFDEF012);
      final theme = ThemeData(
        colorScheme: ColorScheme.fromSeed(seedColor: Colors.purple).copyWith(
          onSurface: primaryForeground,
          onSurfaceVariant: secondaryForeground,
          surfaceContainerHighest: searchSurface,
        ),
      );

      await tester.pumpWidget(
        MaterialApp(
          theme: theme,
          home: const Scaffold(body: SizedBox()),
        ),
      );

      final context = tester.element(find.byType(SizedBox));
      expect(navigationPrimaryForeground(context), primaryForeground);
      expect(navigationSecondaryForeground(context), secondaryForeground);
      expect(navigationSectionForeground(context), secondaryForeground);
      expect(navigationSearchSurface(context), searchSurface);
      expect(
        navigationDivider(context, 0.15),
        primaryForeground.withValues(alpha: 0.15),
      );
    });
  });

  group('isCrewTheme', () {
    test('matches only the Crew pair', () {
      expect(isCrewTheme(crewThemeName), isTrue);
      expect(isCrewTheme(crewDarkThemeName), isTrue);
      expect(isCrewTheme('github-light'), isFalse);
      expect(isCrewTheme(''), isFalse);
    });
  });
}
