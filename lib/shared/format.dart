String formatClock(int ms) {
  final total = ms < 0 ? 0 : ms ~/ 1000;
  final hours = total ~/ 3600;
  final minutes = (total % 3600) ~/ 60;
  final seconds = total % 60;
  String two(int v) => v.toString().padLeft(2, '0');
  if (hours > 0) return '${two(hours)}:${two(minutes)}:${two(seconds)}';
  return '${two(minutes)}:${two(seconds)}';
}

String formatHm(int ms) {
  final total = ms < 0 ? 0 : ms ~/ 1000;
  final hours = total ~/ 3600;
  final minutes = (total % 3600) ~/ 60;
  if (hours == 0 && minutes == 0) {
    return total > 0 ? '<1m' : '0m';
  }
  if (hours == 0) return '${minutes}m';
  if (minutes == 0) return '${hours}h';
  return '${hours}h ${minutes}m';
}

String formatHmCompact(int ms) {
  final total = ms < 0 ? 0 : ms;
  final hours = total / 3600000;
  if (hours >= 10) return '${hours.toStringAsFixed(0)}h';
  if (hours >= 1) return '${hours.toStringAsFixed(1)}h';
  final minutes = total ~/ 60000;
  return '${minutes}m';
}

const _weekdays = ['Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat', 'Sun'];
const _months = [
  'Jan',
  'Feb',
  'Mar',
  'Apr',
  'May',
  'Jun',
  'Jul',
  'Aug',
  'Sep',
  'Oct',
  'Nov',
  'Dec',
];

String formatDayLabel(DateTime date) {
  return '${_weekdays[date.weekday - 1]} ${date.day} ${_months[date.month - 1]}';
}
