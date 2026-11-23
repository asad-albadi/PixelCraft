import 'package:flutter_test/flutter_test.dart';
import 'package:pixelcraft_mobile/animation/playback_clock.dart';

void main() {
  test(
    'Presentation clock honors durations, speed, loops and finite repeat',
    () {
      final clock = PlaybackClock([1, 0], [125, 75], repeat: 2);
      expect(clock.sample(Duration.zero).frame, 1);
      expect(clock.sample(const Duration(milliseconds: 125)).frame, 0);
      expect(clock.sample(const Duration(milliseconds: 200)).frame, 1);
      expect(clock.sample(const Duration(milliseconds: 399)).finished, false);
      expect(clock.sample(const Duration(milliseconds: 400)).finished, true);
      final fast = PlaybackClock([0, 1], [100, 100], speed: 2);
      expect(fast.sample(const Duration(milliseconds: 50)).frame, 1);
      expect(fast.sample(const Duration(milliseconds: 500)).finished, false);
      expect(() => PlaybackClock([0], [0]), throwsArgumentError);
    },
  );
}
