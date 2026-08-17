using System.Diagnostics;
using System.Net.Http.Headers;
using System.Text;
using System.Text.Json;
using Avalonia;
using Avalonia.Controls;
using Avalonia.Controls.Primitives;
using Avalonia.Controls.Templates;
using Avalonia.Layout;
using Avalonia.Media;
using Avalonia.Styling;

namespace Cakify.AvaloniaBench;

public sealed class MainWindow : Window
{
    private readonly Process? _core;
    private readonly ReadyResponse? _ready;
    private readonly TextBlock _status;
    private readonly ListBox _messages;
    private readonly HttpClient _http = new();
    private bool _dark;

    public MainWindow(string[] args)
    {
        Title = "Cakify · Avalonia benchmark";
        Width = 1280;
        Height = 800;
        MinWidth = 960;
        MinHeight = 640;
        Background = Brush("#f7f8fa");

        (_core, _ready) = StartCore(FindArgument(args, "--core-path"), FindArgument(args, "--core-ready-file"));
        var page = _ready is null ? null : FetchPage(_ready, 0, 200);

        _status = new TextBlock
        {
            Text = _ready is null ? "未连接 core · 静态 UI 模式" : "core ready · 已载入首个分页",
            Foreground = Brush("#64717b"),
            FontSize = 12,
            VerticalAlignment = VerticalAlignment.Center,
        };

        _messages = new ListBox
        {
            ItemsSource = Enumerable.Range(0, 10_000).ToArray(),
            Background = Brushes.Transparent,
            BorderThickness = new Thickness(0),
            Padding = new Thickness(24, 16),
            ItemTemplate = new FuncDataTemplate<int>((index, _) => MessageCard(index, page)),
        };

        var root = new Grid
        {
            ColumnDefinitions = new ColumnDefinitions("264,*"),
            RowDefinitions = new RowDefinitions("*"),
        };
        root.Children.Add(BuildSidebar());
        var content = BuildContent();
        Grid.SetColumn(content, 1);
        root.Children.Add(content);
        Content = root;

        Closing += (_, _) => StopCore();
    }

    private Control BuildSidebar()
    {
        var panel = new StackPanel
        {
            Spacing = 12,
            Margin = new Thickness(16),
        };
        panel.Children.Add(new TextBlock
        {
            Text = "Cakify",
            FontSize = 24,
            FontWeight = Avalonia.Media.FontWeight.SemiBold,
            Foreground = Brush("#182026"),
        });
        panel.Children.Add(Card("Benchmark workspace", "Avalonia + C# + Rust"));
        panel.Children.Add(new TextBlock { Text = "会话", Foreground = Brush("#64717b") });
        for (var index = 0; index < 6; index++)
        {
            panel.Children.Add(new Border
            {
                Background = index == 0 ? Brush("#eef1f4") : Brushes.Transparent,
                CornerRadius = new CornerRadius(8),
                Padding = new Thickness(12, 10),
                Child = new TextBlock { Text = index == 0 ? "10k fixture / active" : "Archived conversation" },
            });
        }

        panel.Children.Add(new Border { Height = 1, VerticalAlignment = VerticalAlignment.Stretch });
        panel.Children.Add(new TextBlock
        {
            Text = "同一 Rust core · deterministic fixture",
            Foreground = Brush("#64717b"),
            FontSize = 11,
            TextWrapping = TextWrapping.Wrap,
        });
        return new Border
        {
            Background = Brush("#ffffff"),
            BorderBrush = Brush("#d7dee4"),
            BorderThickness = new Thickness(0, 0, 1, 0),
            Child = panel,
        };
    }

    private Control BuildContent()
    {
        var content = new Grid { RowDefinitions = new RowDefinitions("64,*,152") };
        var header = new Grid { ColumnDefinitions = new ColumnDefinitions("*,Auto") };
        header.Children.Add(new TextBlock
        {
            Text = "New conversation",
            FontSize = 18,
            VerticalAlignment = VerticalAlignment.Center,
            Margin = new Thickness(24, 0),
        });
        var actions = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8, Margin = new Thickness(0, 0, 24, 0) };
        var theme = new Button { Content = "暗色", Padding = new Thickness(12, 8) };
        theme.Click += (_, _) => ToggleTheme(theme);
        var run = new Button { Content = "运行 fixture", Padding = new Thickness(12, 8), Background = Brush("#1f6f78"), Foreground = Brushes.White };
        run.Click += async (_, _) => await RunFixtureAsync();
        actions.Children.Add(theme);
        actions.Children.Add(run);
        Grid.SetColumn(actions, 1);
        header.Children.Add(actions);
        content.Children.Add(new Border { Background = Brush("#ffffff"), BorderBrush = Brush("#d7dee4"), BorderThickness = new Thickness(0, 0, 0, 1), Child = header });
        Grid.SetRow(_messages, 1);
        content.Children.Add(_messages);
        var composer = BuildComposer();
        Grid.SetRow(composer, 2);
        content.Children.Add(composer);
        return content;
    }

    private Control BuildComposer()
    {
        var input = new TextBox
        {
            PlaceholderText = "输入消息… 支持中文 IME",
            AcceptsReturn = true,
            MinHeight = 112,
            TextWrapping = TextWrapping.Wrap,
            Padding = new Thickness(12),
            Background = Brush("#ffffff"),
            BorderBrush = Brush("#d7dee4"),
            CornerRadius = new CornerRadius(8),
        };
        var grid = new Grid { RowDefinitions = new RowDefinitions("*,Auto"), Margin = new Thickness(24, 12, 24, 16) };
        grid.Children.Add(input);
        Grid.SetRow(_status, 1);
        grid.Children.Add(_status);
        return new Border { Background = Brush("#ffffff"), BorderBrush = Brush("#d7dee4"), BorderThickness = new Thickness(0, 1, 0, 0), Child = grid };
    }

    private static Control Card(string title, string subtitle) => new Border
    {
        Background = Brush("#eef1f4"),
        CornerRadius = new CornerRadius(8),
        Padding = new Thickness(12),
        Child = new StackPanel
        {
            Spacing = 4,
            Children =
            {
                new TextBlock { Text = title },
                new TextBlock { Text = subtitle, Foreground = Brush("#64717b"), FontSize = 12 },
            },
        },
    };

    private static Control MessageCard(int index, MessagePage? page)
    {
        var message = page?.Messages.FirstOrDefault(item => item.Index == index);
        var text = message is null ? $"加载 fixture 消息 {index:00000}" : $"{message.Role}  ·  {message.Markdown.Replace('\n', ' ')}";
        return new Border
        {
            Background = index % 4 == 0 ? Brush("#ffffff") : Brush("#eef1f4"),
            BorderBrush = Brush("#d7dee4"),
            BorderThickness = new Thickness(1),
            CornerRadius = new CornerRadius(8),
            Padding = new Thickness(12),
            Margin = new Thickness(0, 0, 0, 8),
            Child = new TextBlock { Text = text, TextWrapping = TextWrapping.Wrap, MaxWidth = 760 },
        };
    }

    private void ToggleTheme(Button button)
    {
        _dark = !_dark;
        RequestedThemeVariant = _dark ? ThemeVariant.Dark : ThemeVariant.Light;
        Background = _dark ? Brush("#15191d") : Brush("#f7f8fa");
        button.Content = _dark ? "亮色" : "暗色";
    }

    private async Task RunFixtureAsync()
    {
        if (_ready is null)
        {
            _status.Text = "core 未连接 · CI 会传入 --core-path";
            return;
        }

        _status.Text = "工具时间线运行中 · 可取消";
        using var request = new HttpRequestMessage(HttpMethod.Post, Endpoint(_ready, "/run/cancel"));
        request.Headers.Add("x-cakify-session", _ready.SessionToken);
        request.Content = new StringContent(JsonSerializer.Serialize(new CancelRequest("avalonia-fixture")), Encoding.UTF8, "application/json");
        try
        {
            await _http.SendAsync(request);
            _status.Text = "工具时间线已发送取消";
        }
        catch (HttpRequestException error)
        {
            _status.Text = $"core 请求失败：{error.Message}";
        }
    }

    private static (Process? Process, ReadyResponse? Ready) StartCore(string? corePath, string? coreReadyFile)
    {
        if (string.IsNullOrWhiteSpace(corePath)) return (null, null);
        var process = new Process
        {
            StartInfo = new ProcessStartInfo(corePath)
            {
                Arguments = string.IsNullOrWhiteSpace(coreReadyFile)
                    ? "--port 0"
                    : $"--port 0 --ready-file \"{coreReadyFile}\"",
                RedirectStandardOutput = true,
                RedirectStandardError = true,
                UseShellExecute = false,
                CreateNoWindow = true,
            },
            EnableRaisingEvents = true,
        };
        try
        {
            process.Start();
            var line = process.StandardOutput.ReadLine();
            var json = line?.StartsWith("CAKIFY_READY ", StringComparison.Ordinal) == true ? line[13..] : null;
            var ready = json is null ? null : JsonSerializer.Deserialize<ReadyResponse>(json);
            return ready is null ? (StopAndNull(process), null) : (process, ready);
        }
        catch (Exception)
        {
            process.Dispose();
            return (null, null);
        }
    }

    private MessagePage? FetchPage(ReadyResponse ready, int offset, int limit)
    {
        using var request = new HttpRequestMessage(HttpMethod.Get, Endpoint(ready, $"/fixture/messages?offset={offset}&limit={limit}"));
        request.Headers.Add("x-cakify-session", ready.SessionToken);
        try
        {
            var response = _http.Send(request);
            response.EnsureSuccessStatusCode();
            return JsonSerializer.Deserialize<MessagePage>(response.Content.ReadAsStringAsync().GetAwaiter().GetResult());
        }
        catch (Exception)
        {
            return null;
        }
    }

    private void StopCore()
    {
        if (_core is { HasExited: false })
        {
            try { _core.Kill(entireProcessTree: true); } catch { /* process may already be gone */ }
        }
        _http.Dispose();
    }

    private static Process? StopAndNull(Process process)
    {
        try { if (!process.HasExited) process.Kill(entireProcessTree: true); } catch { }
        process.Dispose();
        return null;
    }

    private static string Endpoint(ReadyResponse ready, string path) => $"http://127.0.0.1:{ready.Port}{path}";
    private static IBrush Brush(string hex) => new SolidColorBrush(Color.Parse(hex));

    private static string? FindArgument(string[] args, string name)
    {
        for (var index = 0; index < args.Length - 1; index++)
        {
            if (args[index].Equals(name, StringComparison.OrdinalIgnoreCase)) return args[index + 1];
        }
        return null;
    }
}
