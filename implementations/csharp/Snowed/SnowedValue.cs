using System.Text;

namespace Snowed;

/// <summary>
/// The base class for all Snowed value types. Use the static factory methods or
/// typed subclasses to create values. Use pattern matching to inspect them:
/// <code>
/// if (value is SnowedString s) Console.WriteLine(s.Value);
/// </code>
/// </summary>
public abstract class SnowedValue
{
    private protected SnowedValue() { }

    // ── Null ──────────────────────────────────────────────────────────────

    /// <summary>The singleton Null value.</summary>
    public static SnowedNull Null => SnowedNull.Instance;

    // ── Factories ─────────────────────────────────────────────────────────

    public static SnowedBoolean  Create(bool value)           => new(value);
    public static SnowedInt64    Create(long value)           => new(value);
    public static SnowedUInt64   Create(ulong value)          => new(value);
    public static SnowedFloat64  Create(double value)         => new(value);
    public static SnowedDateTime Create(DateTimeOffset value) => new(value);
    public static SnowedBinary   Create(ReadOnlyMemory<byte> value) => new(value);
    public static SnowedBinary   Create(byte[] value)         => new(value);
    public static SnowedString   Create(string value)         => new(value);
    public static SnowedDecimal  Create(decimal value)        => new(value);
    public static SnowedUuid     Create(Guid value)           => new(value);
    public static SnowedArray    Create(IEnumerable<SnowedValue> items) => new(items);
    public static SnowedObject   CreateObject()               => new();
    public static SnowedObject   CreateObject(IEnumerable<KeyValuePair<string, SnowedValue>> pairs) => new(pairs);

    // Nested class trick — all subclasses are sealed and defined below.
    // They are part of the same namespace, not nested, to keep usage clean.
    internal sealed class Seal { }
}

// ── Null ─────────────────────────────────────────────────────────────────────

public sealed class SnowedNull : SnowedValue
{
    public static readonly SnowedNull Instance = new();
    private SnowedNull() { }
    public override string ToString() => "null";
}

// ── Boolean ──────────────────────────────────────────────────────────────────

public sealed class SnowedBoolean : SnowedValue
{
    public bool Value { get; }
    internal SnowedBoolean(bool value) { Value = value; }
    public override string ToString() => Value ? "true" : "false";
}

// ── Int64 ────────────────────────────────────────────────────────────────────

public sealed class SnowedInt64 : SnowedValue
{
    public long Value { get; }
    internal SnowedInt64(long value) { Value = value; }
    public override string ToString() => Value.ToString();
}

// ── UInt64 ───────────────────────────────────────────────────────────────────

public sealed class SnowedUInt64 : SnowedValue
{
    public ulong Value { get; }
    internal SnowedUInt64(ulong value) { Value = value; }
    public override string ToString() => Value.ToString();
}

// ── Float64 ──────────────────────────────────────────────────────────────────

public sealed class SnowedFloat64 : SnowedValue
{
    public double Value { get; }
    internal SnowedFloat64(double value) { Value = value; }
    public override string ToString() => Value.ToString("R");
}

// ── DateTime ─────────────────────────────────────────────────────────────────

public sealed class SnowedDateTime : SnowedValue
{
    /// <summary>Always UTC.</summary>
    public DateTimeOffset Value { get; }
    internal SnowedDateTime(DateTimeOffset value) { Value = value.ToUniversalTime(); }
    public override string ToString() => Value.ToString("O");
}

// ── Binary ───────────────────────────────────────────────────────────────────

public sealed class SnowedBinary : SnowedValue
{
    public ReadOnlyMemory<byte> Value { get; }
    internal SnowedBinary(ReadOnlyMemory<byte> value) { Value = value; }
    internal SnowedBinary(byte[] value) { Value = value; }
    public override string ToString() => $"<binary {Value.Length} bytes>";
}

// ── String ───────────────────────────────────────────────────────────────────

public sealed class SnowedString : SnowedValue
{
    public string Value { get; }
    internal SnowedString(string value) { Value = value; }
    public override string ToString() => Value;
}

// ── Decimal ──────────────────────────────────────────────────────────────────

public sealed class SnowedDecimal : SnowedValue
{
    public decimal Value { get; }
    internal SnowedDecimal(decimal value) { Value = value; }
    public override string ToString() => CanonicalDecimalString(Value);

    /// <summary>
    /// Converts a decimal to the canonical Snowed string form:
    /// invariant period separator, no exponent notation, no trailing zeros.
    /// </summary>
    internal static string CanonicalDecimalString(decimal value)
    {
        if (value == 0m) return "0";
        // "0.############################" — 28 # chars covers the full decimal precision.
        // The # specifier suppresses trailing zeros; the leading 0 ensures "0.5" not ".5".
        return value.ToString("0.############################", System.Globalization.CultureInfo.InvariantCulture);
    }
}

// ── UUID ─────────────────────────────────────────────────────────────────────

public sealed class SnowedUuid : SnowedValue
{
    public Guid Value { get; }
    internal SnowedUuid(Guid value) { Value = value; }
    public override string ToString() => Value.ToString();

    /// <summary>
    /// Returns the 16 bytes of this UUID in RFC 4122 byte order (big-endian throughout).
    /// .NET's Guid uses a mixed-endian layout; this method corrects for that.
    /// </summary>
    internal byte[] ToRfc4122Bytes()
    {
        byte[] bytes = Value.ToByteArray();   // mixed-endian .NET layout
        Array.Reverse(bytes, 0, 4);           // Data1: LE → BE
        Array.Reverse(bytes, 4, 2);           // Data2: LE → BE
        Array.Reverse(bytes, 6, 2);           // Data3: LE → BE
        // bytes[8..15] (Data4) are already in correct order
        return bytes;
    }

    /// <summary>Creates a SnowedUuid from 16 bytes in RFC 4122 byte order.</summary>
    internal static SnowedUuid FromRfc4122Bytes(ReadOnlySpan<byte> bytes)
    {
        if (bytes.Length != 16) throw new SnowedException($"UUID payload must be exactly 16 bytes, got {bytes.Length}");
        byte[] b = bytes.ToArray();
        Array.Reverse(b, 0, 4);
        Array.Reverse(b, 4, 2);
        Array.Reverse(b, 6, 2);
        return new SnowedUuid(new Guid(b));
    }
}

// ── Array ────────────────────────────────────────────────────────────────────

public sealed class SnowedArray : SnowedValue
{
    public IList<SnowedValue> Items { get; }

    internal SnowedArray() { Items = new List<SnowedValue>(); }
    internal SnowedArray(IEnumerable<SnowedValue> items) { Items = new List<SnowedValue>(items); }

    public void Add(SnowedValue item) => Items.Add(item);
    public SnowedValue this[int index] => Items[index];
    public int Count => Items.Count;
    public override string ToString() => $"[{string.Join(", ", Items)}]";
}

// ── Object ───────────────────────────────────────────────────────────────────

public sealed class SnowedObject : SnowedValue
{
    private readonly Dictionary<string, SnowedValue> _props;

    internal SnowedObject() { _props = new Dictionary<string, SnowedValue>(); }
    internal SnowedObject(IEnumerable<KeyValuePair<string, SnowedValue>> pairs)
    {
        _props = new Dictionary<string, SnowedValue>(pairs);
    }

    public SnowedValue this[string key]
    {
        get => _props[key];
        set => _props[key] = value;
    }

    public bool TryGetValue(string key, out SnowedValue value)  => _props.TryGetValue(key, out value!);
    public bool ContainsKey(string key)                         => _props.ContainsKey(key);
    public ICollection<string> Keys                             => _props.Keys;
    public ICollection<SnowedValue> Values                      => _props.Values;
    public int Count                                            => _props.Count;
    public IEnumerable<KeyValuePair<string, SnowedValue>> Pairs => _props;

    public override string ToString()
    {
        var sb = new StringBuilder("{");
        bool first = true;
        foreach (var kv in _props)
        {
            if (!first) sb.Append(", ");
            sb.Append('"').Append(kv.Key).Append("\": ").Append(kv.Value);
            first = false;
        }
        sb.Append('}');
        return sb.ToString();
    }
}
