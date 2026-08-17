import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:flutter/material.dart';

const canvasLight = Color(0xfff7f8fa);
const surfaceLight = Color(0xffffffff);
const mutedSurfaceLight = Color(0xffeef1f4);
const textLight = Color(0xff182026);
const mutedTextLight = Color(0xff64717b);
const accentLight = Color(0xff1f6f78);
const borderLight = Color(0xffd7dee4);

const canvasDark = Color(0xff15191d);
const surfaceDark = Color(0xff20262b);
const mutedSurfaceDark = Color(0xff2a3238);
const textDark = Color(0xffe8edf0);
const mutedTextDark = Color(0xffa7b2b9);
const accentDark = Color(0xff66c2c8);
const borderDark = Color(0xff3a454d);

Future<void> main(List<String> args) async {
  final core = await CoreSession.start(_argument(args, '--core-path'), _argument(args, '--core-ready-file'));
  runApp(BenchApp(core: core));
}

class CoreSession {
  CoreSession(this.process, this.port, this.token, this.fixtureHash);

  final Process? process;
  final int? port;
  final String? token;
  final String? fixtureHash;

  bool get ready => process != null && port != null && token != null;

  static Future<CoreSession> start(String? path, String? readyFile) async {
    if (path == null || path.isEmpty) return CoreSession(null, null, null, null);
    try {
      final processArgs = <String>['--port', '0'];
      if (readyFile != null && readyFile.isNotEmpty) processArgs.addAll(['--ready-file', readyFile]);
      final process = await Process.start(path, processArgs);
      final line = await process.stdout
          .transform(utf8.decoder)
          .transform(const LineSplitter())
          .first
          .timeout(const Duration(seconds: 5));
      if (!line.startsWith('CAKIFY_READY ')) {
        process.kill();
        return CoreSession(null, null, null, null);
      }
      final ready = jsonDecode(line.substring('CAKIFY_READY '.length)) as Map<String, dynamic>;
      return CoreSession(
        process,
        (ready['port'] as num).toInt(),
        ready['session_token'] as String,
        ready['fixture_hash'] as String,
      );
    } catch (_) {
      return CoreSession(null, null, null, null);
    }
  }

  Future<List<Map<String, dynamic>>> fetchMessages() async {
    if (!ready) return const [];
    final client = HttpClient();
    try {
      final request = await client.getUrl(Uri.parse('http://127.0.0.1:$port/fixture/messages?offset=0&limit=200'));
      request.headers.set('x-cakify-session', token!);
      final response = await request.close();
      final body = await response.transform(utf8.decoder).join();
      final payload = jsonDecode(body) as Map<String, dynamic>;
      return (payload['messages'] as List<dynamic>).cast<Map<String, dynamic>>();
    } catch (_) {
      return const [];
    } finally {
      client.close(force: true);
    }
  }

  Future<bool> cancel(String runId) async {
    if (!ready) return false;
    final client = HttpClient();
    try {
      final request = await client.postUrl(Uri.parse('http://127.0.0.1:$port/run/cancel'));
      request.headers.set('x-cakify-session', token!);
      request.headers.contentType = ContentType.json;
      request.write(jsonEncode({'run_id': runId}));
      final response = await request.close();
      return response.statusCode == HttpStatus.ok;
    } catch (_) {
      return false;
    } finally {
      client.close(force: true);
    }
  }

  void close() {
    process?.kill();
  }
}

class BenchApp extends StatefulWidget {
  const BenchApp({required this.core, super.key});

  final CoreSession core;

  @override
  State<BenchApp> createState() => _BenchAppState();
}

class _BenchAppState extends State<BenchApp> {
  ThemeMode _themeMode = ThemeMode.light;
  String _status = '等待 core';
  List<Map<String, dynamic>> _messages = const [];

  @override
  void initState() {
    super.initState();
    _status = widget.core.ready ? 'core ready · 载入首个分页中' : '未连接 core · 静态 UI 模式';
    _loadMessages();
  }

  Future<void> _loadMessages() async {
    final messages = await widget.core.fetchMessages();
    if (!mounted) return;
    setState(() {
      _messages = messages;
      _status = widget.core.ready ? 'core ready · 已载入首个分页' : _status;
    });
  }

  Future<void> _runFixture() async {
    setState(() => _status = widget.core.ready ? '工具时间线运行中 · 可取消' : 'core 未连接');
    final accepted = await widget.core.cancel('flutter-fixture');
    if (!mounted) return;
    setState(() => _status = accepted ? '工具时间线已发送取消' : 'core 未接受取消');
  }

  @override
  void dispose() {
    widget.core.close();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final isDark = _themeMode == ThemeMode.dark;
    final text = isDark ? textDark : textLight;
    final mutedText = isDark ? mutedTextDark : mutedTextLight;
    final surface = isDark ? surfaceDark : surfaceLight;
    final mutedSurface = isDark ? mutedSurfaceDark : mutedSurfaceLight;
    final border = isDark ? borderDark : borderLight;
    final accent = isDark ? accentDark : accentLight;

    return MaterialApp(
      debugShowCheckedModeBanner: false,
      themeMode: _themeMode,
      theme: ThemeData(useMaterial3: true, scaffoldBackgroundColor: canvasLight, colorScheme: ColorScheme.fromSeed(seedColor: accentLight)),
      darkTheme: ThemeData(useMaterial3: true, scaffoldBackgroundColor: canvasDark, colorScheme: ColorScheme.fromSeed(seedColor: accentDark, brightness: Brightness.dark)),
      home: Scaffold(
        body: Row(
          children: [
            SizedBox(width: 264, child: _buildSidebar(surface, mutedSurface, text, mutedText, border)),
            Expanded(child: _buildContent(surface, mutedSurface, text, mutedText, border, accent)),
          ],
        ),
      ),
    );
  }

  Widget _buildSidebar(Color surface, Color mutedSurface, Color text, Color mutedText, Color border) {
    return Container(
      decoration: BoxDecoration(color: surface, border: Border(right: BorderSide(color: border))),
      padding: const EdgeInsets.all(16),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Text('Cakify', style: TextStyle(fontSize: 24, fontWeight: FontWeight.w600, color: text)),
          const SizedBox(height: 12),
          Container(
            padding: const EdgeInsets.all(12),
            decoration: BoxDecoration(color: mutedSurface, borderRadius: BorderRadius.circular(8)),
            child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
              Text('Benchmark workspace', style: TextStyle(color: text)),
              const SizedBox(height: 4),
              Text('Flutter + Rust', style: TextStyle(color: mutedText, fontSize: 12)),
            ]),
          ),
          const SizedBox(height: 16),
          Text('会话', style: TextStyle(color: mutedText)),
          const SizedBox(height: 8),
          ...List.generate(6, (index) => Container(
                margin: const EdgeInsets.only(bottom: 4),
                padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 10),
                decoration: BoxDecoration(color: index == 0 ? mutedSurface : Colors.transparent, borderRadius: BorderRadius.circular(8)),
                child: Text(index == 0 ? '10k fixture / active' : 'Archived conversation', style: TextStyle(color: text, fontSize: 13)),
              )),
          const Spacer(),
          Text('同一 Rust core · deterministic fixture', style: TextStyle(color: mutedText, fontSize: 11)),
        ],
      ),
    );
  }

  Widget _buildContent(Color surface, Color mutedSurface, Color text, Color mutedText, Color border, Color accent) {
    return Column(
      children: [
        Container(
          height: 64,
          padding: const EdgeInsets.symmetric(horizontal: 24),
          decoration: BoxDecoration(color: surface, border: Border(bottom: BorderSide(color: border))),
          child: Row(children: [
            Text('New conversation', style: TextStyle(color: text, fontSize: 18)),
            const Spacer(),
            OutlinedButton(
              onPressed: () => setState(() => _themeMode = _themeMode == ThemeMode.light ? ThemeMode.dark : ThemeMode.light),
              child: Text(_themeMode == ThemeMode.light ? '暗色' : '亮色'),
            ),
            const SizedBox(width: 8),
            FilledButton(onPressed: _runFixture, style: FilledButton.styleFrom(backgroundColor: accent), child: const Text('运行 fixture')),
          ]),
        ),
        Expanded(
          child: ListView.builder(
            itemCount: 10000,
            padding: const EdgeInsets.symmetric(horizontal: 24, vertical: 16),
            itemBuilder: (context, index) => _messageCard(index, mutedSurface, surface, text, border),
          ),
        ),
        Container(
          padding: const EdgeInsets.fromLTRB(24, 12, 24, 16),
          decoration: BoxDecoration(color: surface, border: Border(top: BorderSide(color: border))),
          child: Column(children: [
            TextField(
              minLines: 4,
              maxLines: 4,
              decoration: InputDecoration(hintText: '输入消息… 支持中文 IME', filled: true, fillColor: surface, border: OutlineInputBorder(borderRadius: BorderRadius.circular(8))),
            ),
            const SizedBox(height: 8),
            Row(children: [Text(_status, style: TextStyle(color: mutedText, fontSize: 12)), const Spacer(), Icon(Icons.attach_file, size: 18, color: mutedText), const SizedBox(width: 8), Icon(Icons.send, color: accent)]),
          ]),
        ),
      ],
    );
  }

  Widget _messageCard(int index, Color mutedSurface, Color surface, Color text, Color border) {
    final message = index < _messages.length ? _messages[index] : null;
    final raw = message?['markdown'] as String? ?? '加载 fixture 消息 ${index.toString().padLeft(5, '0')}';
    final role = message?['role'] as String? ?? 'fixture';
    return Container(
      margin: const EdgeInsets.only(bottom: 8),
      padding: const EdgeInsets.all(12),
      decoration: BoxDecoration(color: index % 4 == 0 ? surface : mutedSurface, border: Border.all(color: border), borderRadius: BorderRadius.circular(8)),
      child: Text('$role  ·  ${raw.replaceAll('\n', ' ')}', style: TextStyle(color: text, height: 1.5)),
    );
  }
}

String? _argument(List<String> args, String name) {
  final index = args.indexOf(name);
  return index >= 0 && index + 1 < args.length ? args[index + 1] : null;
}
