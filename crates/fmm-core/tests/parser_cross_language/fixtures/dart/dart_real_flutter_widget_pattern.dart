
import 'package:flutter/material.dart';
import 'package:provider/provider.dart';

class CounterApp extends StatelessWidget {
  Widget build(BuildContext context) {
    return MaterialApp(home: CounterPage());
  }
}

class CounterPage extends StatefulWidget {
  State<CounterPage> createState() => _CounterPageState();
}

class _CounterPageState extends State<CounterPage> {
  int _counter = 0;

  void _increment() {
    setState(() { _counter++; });
  }

  Widget build(BuildContext context) {
    return Scaffold();
  }
}

class CounterModel extends ChangeNotifier {
  int _count = 0;
  int get count => _count;

  void increment() {
    _count++;
    notifyListeners();
  }
}

enum AppRoute { home, settings, profile }

typedef WidgetCallback = void Function(BuildContext);
