/// Presentation timing for the frame order supplied by the shared Rust engine.
class PlaybackClock {
  final List<int> frames, durations;
  final double speed;
  final int repeat;
  late final int cycle = durations.fold(0, (a, b) => a + b) * 1000;
  PlaybackClock(
    this.frames,
    this.durations, {
    this.speed = 1,
    this.repeat = 0,
  }) {
    if (frames.isEmpty ||
        frames.length > 128 ||
        frames.length != durations.length ||
        frames.any((f) => f < 0 || f >= 64) ||
        durations.any((d) => d < 1 || d > 65535) ||
        !speed.isFinite ||
        speed < .25 ||
        speed > 4 ||
        repeat < 0 ||
        repeat > 65535) {
      throw ArgumentError('Invalid animation timeline');
    }
  }
  ({int frame, Duration wait, bool finished}) sample(Duration elapsed) {
    final time = (elapsed.inMicroseconds * speed).floor();
    if (repeat > 0 && time >= cycle * repeat) {
      return (frame: frames.last, wait: Duration.zero, finished: true);
    }
    var within = time % cycle;
    for (var i = 0; i < frames.length; i++) {
      final duration = durations[i] * 1000;
      if (within < duration) {
        return (
          frame: frames[i],
          wait: Duration(
            microseconds: ((duration - within) / speed).ceil().clamp(
              1000,
              65535000,
            ),
          ),
          finished: false,
        );
      }
      within -= duration;
    }
    throw StateError('Invalid animation phase');
  }
}
