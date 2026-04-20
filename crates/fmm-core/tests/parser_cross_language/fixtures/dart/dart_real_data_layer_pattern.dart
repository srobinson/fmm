
import 'dart:convert';
import 'package:http/http.dart' as http;

abstract class Repository<T> {
  Future<T?> findById(String id);
  Future<List<T>> findAll();
  Future<void> save(T entity);
}

class UserRepository extends Repository<Map<String, dynamic>> {
  final String baseUrl;

  UserRepository({required this.baseUrl});

  Future<Map<String, dynamic>?> findById(String id) async {
    return null;
  }

  Future<List<Map<String, dynamic>>> findAll() async {
    return [];
  }

  Future<void> save(Map<String, dynamic> entity) async {}
}

class ApiClient {
  static const String _baseUrl = 'https://api.example.com';

  Future<Map<String, dynamic>> get(String path) async {
    return {};
  }
}

typedef JsonMap = Map<String, dynamic>;

void _initializeClient() {}
