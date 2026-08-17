using System.Text.Json.Serialization;

namespace Cakify.AvaloniaBench;

public sealed record ReadyResponse(
    [property: JsonPropertyName("port")] int Port,
    [property: JsonPropertyName("protocol_version")] string ProtocolVersion,
    [property: JsonPropertyName("fixture_hash")] string FixtureHash,
    [property: JsonPropertyName("session_token")] string SessionToken,
    [property: JsonPropertyName("pid")] int Pid);

public sealed record MessageRecord(
    [property: JsonPropertyName("id")] string Id,
    [property: JsonPropertyName("index")] int Index,
    [property: JsonPropertyName("role")] string Role,
    [property: JsonPropertyName("markdown")] string Markdown,
    [property: JsonPropertyName("has_image")] bool HasImage);

public sealed record MessagePage(
    [property: JsonPropertyName("fixture_hash")] string FixtureHash,
    [property: JsonPropertyName("offset")] int Offset,
    [property: JsonPropertyName("total")] int Total,
    [property: JsonPropertyName("messages")] IReadOnlyList<MessageRecord> Messages);

public sealed record CancelRequest(
    [property: JsonPropertyName("run_id")] string RunId);
