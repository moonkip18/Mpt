// Validates the GitHub Actions workflow parses as YAML. Run: dart run tool/check_workflow.dart
// ignore_for_file: depend_on_referenced_packages
import 'dart:io';

import 'package:yaml/yaml.dart';

void main() {
  final doc =
      loadYaml(File('.github/workflows/windows-build.yml').readAsStringSync());
  final jobs = (doc as YamlMap)['jobs'] as YamlMap;
  stdout.writeln('workflow ok: ${doc['name']} -> jobs ${jobs.keys.join(', ')}');
  final steps = ((jobs['build'] as YamlMap)['steps'] as YamlList).length;
  stdout.writeln('steps: $steps');
}
