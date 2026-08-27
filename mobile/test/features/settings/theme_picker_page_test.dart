import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';
import 'package:crew/features/settings/accent_picker_page.dart';
import 'package:crew/features/settings/theme_picker_page.dart';
import 'package:crew/features/settings/settings_page.dart';
import 'package:crew/shared/theme/theme.dart';
import 'package:shared_preferences/shared_preferences.dart';

import '../../helpers/widget_helpers.dart';

Future<SharedPreferences> _prefs(Map<String, Object> initial) async {
  SharedPreferences.setMockInitialValues(initial);
  return SharedPreferences.getInstance();
}

Future<void> _pumpPicker(
  WidgetTester tester,
  Widget page, {
  Map<String, Object> prefs = const {},
}) async {
  final instance = await _prefs(prefs);
  await tester.pumpWidget(
    WidgetHelpers.testable(
      child: page,
      overrides: [savedPrefsProvider.overrideWithValue(instance)],
    ),
  );
  await tester.pumpAndSettle();
}

Future<void> _search(WidgetTester tester, String query) async {
  await tester.enterText(find.byType(TextField), query);
  await tester.pumpAndSettle();
}

void main() {
  group('ThemePickerPage', () {
    testWidgets('system mode offers pairs under their stripped label', (
      tester,
    ) async {
      await _pumpPicker(tester, const ThemePickerPage());

      await _search(tester, 'github');

      // Paired labels drop the brightness token — one row stands for both halves.
      expect(find.text('Github'), findsOneWidget);
      expect(find.text('Github Default'), findsOneWidget);
      expect(find.text('Github Light'), findsNothing);
      expect(find.text('Github Dark'), findsNothing);
    });

    testWidgets('system mode hides themes with no counterpart', (tester) async {
      await _pumpPicker(tester, const ThemePickerPage());

      // 'snazzy-light' is light but unpaired, so it cannot follow the OS.
      await _search(tester, 'snazzy');

      expect(find.text('No themes found'), findsOneWidget);
    });

    testWidgets('system mode normalizes a stored unpaired theme', (
      tester,
    ) async {
      final instance = await _prefs({'crew_color_scheme': 'snazzy-light'});
      await tester.pumpWidget(
        WidgetHelpers.testable(
          child: const ThemePickerPage(),
          overrides: [savedPrefsProvider.overrideWithValue(instance)],
        ),
      );
      await tester.pumpAndSettle();

      expect(
        instance.getString('crew_color_scheme'),
        themeGroups().paired.first.name,
      );
    });

    testWidgets('light mode lists light themes by their full name', (
      tester,
    ) async {
      await _pumpPicker(
        tester,
        const ThemePickerPage(),
        prefs: {'crew_theme_mode': 'light'},
      );

      await _search(tester, 'github');

      expect(find.text('Github Light'), findsOneWidget);
      expect(find.text('Github Light Default'), findsOneWidget);
      expect(find.text('Github Dark'), findsNothing);
    });

    testWidgets('light mode includes unpaired light themes', (tester) async {
      await _pumpPicker(
        tester,
        const ThemePickerPage(),
        prefs: {'crew_theme_mode': 'light'},
      );

      await _search(tester, 'snazzy');

      expect(find.text('Snazzy Light'), findsOneWidget);
    });

    testWidgets('dark mode lists dark themes only', (tester) async {
      await _pumpPicker(
        tester,
        const ThemePickerPage(),
        prefs: {'crew_theme_mode': 'dark'},
      );

      await _search(tester, 'github');

      expect(find.text('Github Dark'), findsOneWidget);
      expect(find.text('Github Light'), findsNothing);
    });

    testWidgets('checks the row matching the stored selection', (tester) async {
      await _pumpPicker(
        tester,
        const ThemePickerPage(),
        prefs: {'crew_theme_mode': 'light', 'crew_color_scheme': 'nord'},
      );

      await _search(tester, 'nord');

      // 'nord' is dark, so Light mode renders its light fallback instead — the
      // checkmark must follow what is applied, not the raw stored value.
      expect(find.byIcon(LucideIcons.check), findsNothing);
    });

    testWidgets('either half of a pair checks the same system row', (
      tester,
    ) async {
      await _pumpPicker(
        tester,
        const ThemePickerPage(),
        prefs: {'crew_color_scheme': 'github-dark'},
      );

      await _search(tester, 'github');

      final checked = tester
          .widgetList<ListTile>(find.byType(ListTile))
          .where((tile) => tile.trailing != null);
      expect(checked, hasLength(1));
      expect(find.text('Github'), findsOneWidget);
    });

    testWidgets('tapping a theme persists the selection', (tester) async {
      final instance = await _prefs({'crew_theme_mode': 'dark'});
      await tester.pumpWidget(
        WidgetHelpers.testable(
          child: const ThemePickerPage(),
          overrides: [savedPrefsProvider.overrideWithValue(instance)],
        ),
      );
      await tester.pumpAndSettle();

      await _search(tester, 'nord');
      await tester.tap(find.text('Nord'));
      await tester.pumpAndSettle();

      expect(instance.getString('crew_color_scheme'), 'nord');
    });
  });

  group('Crew accent behavior', () {
    testWidgets('settings hides accent navigation for Crew', (tester) async {
      await _pumpPicker(
        tester,
        SettingsPage(
          profileHeader: const SizedBox.shrink(),
          invitePageBuilder: (_) => const SizedBox.shrink(),
          identityRecoveryPageBuilder: (_) => const SizedBox.shrink(),
        ),
        prefs: {'crew_color_scheme': 'crew', 'crew_accent_color': 4},
      );

      expect(find.text('Accent color'), findsNothing);
    });

    testWidgets('settings restores accent navigation away from Crew', (
      tester,
    ) async {
      await _pumpPicker(
        tester,
        SettingsPage(
          profileHeader: const SizedBox.shrink(),
          invitePageBuilder: (_) => const SizedBox.shrink(),
          identityRecoveryPageBuilder: (_) => const SizedBox.shrink(),
        ),
        prefs: {
          'crew_theme_mode': 'light',
          'crew_color_scheme': 'github-light',
          'crew_accent_color': 4,
        },
      );

      expect(find.text('Accent color'), findsOneWidget);
    });
  });

  group('AccentPickerPage', () {
    testWidgets('lists every accent and checks the stored one', (tester) async {
      await _pumpPicker(
        tester,
        const AccentPickerPage(),
        prefs: {'crew_accent_color': 2},
      );

      for (final accent in accentColors) {
        expect(find.text(accent.name), findsOneWidget);
      }
      expect(find.byIcon(LucideIcons.check), findsOneWidget);
    });

    testWidgets('tapping an accent persists the index', (tester) async {
      final instance = await _prefs(const <String, Object>{});
      await tester.pumpWidget(
        WidgetHelpers.testable(
          child: const AccentPickerPage(),
          overrides: [savedPrefsProvider.overrideWithValue(instance)],
        ),
      );
      await tester.pumpAndSettle();

      await tester.tap(find.text('Green'));
      await tester.pumpAndSettle();

      expect(
        instance.getInt('crew_accent_color'),
        accentColors.indexWhere((a) => a.name == 'Green'),
      );
    });
  });
}
