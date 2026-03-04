// Sample Dart file for fmm parser validation
// Covers: classes, abstract classes, mixins, enums, extensions,
// typedefs, functions, variables, imports, privacy convention

import 'package:flutter/material.dart';
import 'package:http/http.dart' as http;
import 'dart:async';
import 'dart:convert';
import './relative_file.dart';
import '../utils/helpers.dart';

// Public classes

class NetworkManager {
  Future<void> fetchData() async {}

  void _privateMethod() {}

  void cancelAll() {}
}

abstract class BaseWidget extends StatelessWidget {
  Widget build(BuildContext context);
}

class UserProfile {
  final String name;
  final String email;

  UserProfile({required this.name, required this.email});
}

// Private class (underscore prefix)
class _InternalHelper {
  void doWork() {}
}

// Mixin

mixin Loggable {
  void log(String message) {
    print(message);
  }
}

mixin Cacheable on Object {
  void invalidateCache() {}
}

// Enum

enum Direction { north, south, east, west }

enum HttpStatus {
  ok(200),
  notFound(404),
  serverError(500);

  final int code;
  const HttpStatus(this.code);
}

// Extension

extension StringExtension on String {
  String capitalize() => '${this[0].toUpperCase()}${substring(1)}';

  bool get isBlank => trim().isEmpty;
}

extension IntExtension on int {
  bool get isEven2 => this % 2 == 0;
}

// Typedef

typedef Callback = void Function(String);
typedef JsonMap = Map<String, dynamic>;

typedef _PrivateCallback = void Function(int);

// Top-level functions

void globalFunction() {}

String processData(List<String> items) {
  return items.join(', ');
}

void _privateFunction() {}

Future<void> asyncOperation() async {}

// Top-level variables

final String appVersion = '1.0.0';
const int maxRetries = 3;
var isDebugMode = false;

final _privateVar = 'hidden';
