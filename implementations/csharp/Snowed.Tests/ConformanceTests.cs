using System.Globalization;
using System.Text.Json;
using Snowed;
using Xunit;

namespace Snowed.Tests;

/// <summary>
/// Runs the shared cross-implementation conformance vectors in <c>tests/conformance/vectors/</c>.
/// See <c>tests/conformance/README.md</c> for the vector format and runner rules.
/// </summary>
public class ConformanceTests
{
    private sealed record VectorCase(string Id, string Hex, JsonElement? Decoded, bool Error, bool Canonical);

    private static readonly Lazy<IReadOnlyDictionary<string, VectorCase>> Cases = new(LoadAllCases);

    public static TheoryData<string> CaseIds()
    {
        var data = new TheoryData<string>();
        foreach (var id in Cases.Value.Keys) data.Add(id);
        return data;
    }

    [Fact]
    public void VectorsArePresent() =>
        Assert.True(Cases.Value.Count > 0, "No conformance vectors found in tests/conformance/vectors");

    [Theory]
    [MemberData(nameof(CaseIds))]
    public void Vector(string id)
    {
        var c = Cases.Value[id];
        var bytes = Convert.FromHexString(c.Hex.Replace(" ", ""));

        if (c.Error)
        {
            Assert.ThrowsAny<SnowedException>(() => SnowedSerializer.DeserializeValue(bytes));
            return;
        }

        var expected = c.Decoded ?? throw new InvalidOperationException($"{id}: case has neither decoded nor error");

        var decoded = SnowedSerializer.DeserializeValue(bytes);
        AssertMatches(decoded, expected, "$", id, "decode");

        var reencoded = SnowedSerializer.Encode(decoded);
        AssertMatches(SnowedSerializer.DeserializeValue(reencoded), expected, "$", id, "round-trip");

        if (c.Canonical)
            Assert.True(bytes.AsSpan().SequenceEqual(reencoded),
                $"{id}: re-encoded bytes differ\n  expected: {c.Hex}\n  actual:   {Hex(reencoded)}");
    }

    // ── Comparison ───────────────────────────────────────────────────────────

    private static void AssertMatches(SnowedValue actual, JsonElement expected, string path, string id, string stage)
    {
        var type = expected.GetProperty("type").GetString();
        var value = expected.GetProperty("value");
        void Fail(string detail) =>
            Assert.Fail($"{id}: {stage} mismatch at {path}: expected {type} {detail}, got {actual.GetType().Name} {actual}");

        switch (type, actual)
        {
            case ("null", SnowedNull):
                return;

            case ("bool", SnowedBoolean b):
                if (b.Value != value.GetBoolean()) Fail(value.GetRawText());
                return;

            case ("int64", SnowedInt64 i):
                if (i.Value != long.Parse(IntegerText(value), CultureInfo.InvariantCulture)) Fail(value.GetRawText());
                return;

            case ("uint64", SnowedUInt64 u):
                if (u.Value != ulong.Parse(IntegerText(value), CultureInfo.InvariantCulture)) Fail(value.GetRawText());
                return;

            case ("float64", SnowedFloat64 f):
            {
                var e = ParseDouble(value);
                var ok = double.IsNaN(e)
                    ? double.IsNaN(f.Value)
                    : BitConverter.DoubleToInt64Bits(e) == BitConverter.DoubleToInt64Bits(f.Value);
                if (!ok) Fail(value.GetRawText());
                return;
            }

            case ("datetime", SnowedDateTime d):
            {
                var ms = (d.Value - DateTimeOffset.UnixEpoch).TotalMilliseconds;
                // DateTimeOffset has 100 ns resolution; the byte-exact check catches real precision loss.
                if (Math.Abs(ms - ParseDouble(value)) >= 0.001) Fail($"{value.GetRawText()} ms (actual {ms} ms)");
                return;
            }

            case ("binary", SnowedBinary bin):
            {
                var e = Convert.FromBase64String(value.GetString()!);
                if (!bin.Value.Span.SequenceEqual(e)) Fail(Hex(e));
                return;
            }

            case ("string", SnowedString s):
                if (s.Value != value.GetString()) Fail(value.GetRawText());
                return;

            case ("decimal", SnowedDecimal dec):
                if (dec.ToString() != value.GetString()) Fail(value.GetRawText());
                return;

            case ("uuid", SnowedUuid g):
                if (g.Value.ToString("D") != value.GetString()) Fail(value.GetRawText());
                return;

            case ("array", SnowedArray arr):
            {
                var items = value.EnumerateArray().ToList();
                if (items.Count != arr.Count) Fail($"with {items.Count} elements");
                for (var i = 0; i < items.Count; i++)
                    AssertMatches(arr[i], items[i], $"{path}[{i}]", id, stage);
                return;
            }

            case ("object", SnowedObject obj):
            {
                var props = value.EnumerateObject().ToList();
                var expectedKeys = props.Select(p => p.Name).OrderBy(k => k, StringComparer.Ordinal);
                var actualKeys = obj.Keys.OrderBy(k => k, StringComparer.Ordinal);
                if (!expectedKeys.SequenceEqual(actualKeys)) Fail($"with keys [{string.Join(", ", expectedKeys)}]");
                foreach (var p in props)
                    AssertMatches(obj[p.Name], p.Value, $"{path}.{p.Name}", id, stage);
                return;
            }

            default:
                Fail("(wrong type)");
                return;
        }
    }

    /// <summary>Integers are read from their exact JSON text, never through a double.</summary>
    private static string IntegerText(JsonElement value) =>
        value.ValueKind == JsonValueKind.String ? value.GetString()! : value.GetRawText();

    private static double ParseDouble(JsonElement value) => value.ValueKind switch
    {
        JsonValueKind.Number => double.Parse(value.GetRawText(), NumberStyles.Float, CultureInfo.InvariantCulture),
        JsonValueKind.String => value.GetString() switch
        {
            "Infinity"  => double.PositiveInfinity,
            "-Infinity" => double.NegativeInfinity,
            "NaN"       => double.NaN,
            "-0"        => -0.0,
            var s       => throw new InvalidOperationException($"Unrecognised float string '{s}'"),
        },
        _ => throw new InvalidOperationException($"Unrecognised float value {value.GetRawText()}"),
    };

    private static string Hex(ReadOnlySpan<byte> bytes) =>
        string.Join(" ", bytes.ToArray().Select(b => b.ToString("X2")));

    // ── Vector loading ───────────────────────────────────────────────────────

    private static string VectorsDirectory()
    {
        for (var dir = new DirectoryInfo(AppContext.BaseDirectory); dir != null; dir = dir.Parent)
        {
            var candidate = Path.Combine(dir.FullName, "tests", "conformance", "vectors");
            if (Directory.Exists(candidate)) return candidate;
        }
        throw new DirectoryNotFoundException("Could not locate tests/conformance/vectors above " + AppContext.BaseDirectory);
    }

    private static IReadOnlyDictionary<string, VectorCase> LoadAllCases()
    {
        var result = new SortedDictionary<string, VectorCase>(StringComparer.Ordinal);
        foreach (var file in Directory.GetFiles(VectorsDirectory(), "*.json").OrderBy(f => f, StringComparer.Ordinal))
        {
            using var doc = JsonDocument.Parse(File.ReadAllText(file));
            var suite = doc.RootElement.GetProperty("suite").GetString()!;
            foreach (var c in doc.RootElement.GetProperty("cases").EnumerateArray())
            {
                var id = $"{suite}/{c.GetProperty("name").GetString()}";
                var error = c.TryGetProperty("error", out var e) && e.GetBoolean();
                var canonical = !c.TryGetProperty("canonical", out var k) || k.GetBoolean();
                JsonElement? decoded = c.TryGetProperty("decoded", out var d) ? d.Clone() : null;
                result.Add(id, new VectorCase(id, c.GetProperty("hex").GetString()!, decoded, error, canonical));
            }
        }
        return result;
    }
}
