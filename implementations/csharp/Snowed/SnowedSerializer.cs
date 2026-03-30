using Snowed.Internal;

namespace Snowed;

/// <summary>
/// Serializes and deserializes CLR objects to and from the Snowed binary format.
/// </summary>
/// <remarks>
/// The synchronous <c>byte[]</c>-based API mirrors <c>System.Text.Json.JsonSerializer</c>.
/// Stream-based and async overloads suit the network and ASP.NET Core use cases.
///
/// <b>Future:</b> A source-generation path (<c>SnowedSerializerContext</c>) is planned
/// to support AOT/trimmed builds without reflection.
/// </remarks>
public static class SnowedSerializer
{
    // ── Serialize ─────────────────────────────────────────────────────────

    /// <summary>Serializes <paramref name="value"/> to a Snowed-encoded byte array.</summary>
    public static byte[] Serialize<T>(T value, SnowedSerializerOptions? options = null)
    {
        var ms = new MemoryStream();
        SerializeToStream(ms, value, typeof(T), options);
        return ms.ToArray();
    }

    /// <summary>Serializes <paramref name="value"/> to <paramref name="stream"/> synchronously.</summary>
    public static void Serialize<T>(Stream stream, T value, SnowedSerializerOptions? options = null)
        => SerializeToStream(stream, value, typeof(T), options);

    /// <summary>Serializes <paramref name="value"/> to <paramref name="stream"/> asynchronously.</summary>
    public static async Task SerializeAsync<T>(Stream stream, T value,
        SnowedSerializerOptions? options = null, CancellationToken cancellationToken = default)
    {
        // Buffer into memory then flush — true streaming serialisation is a future enhancement.
        var ms = new MemoryStream();
        SerializeToStream(ms, value, typeof(T), options);
        ms.Position = 0;
        await ms.CopyToAsync(stream, cancellationToken).ConfigureAwait(false);
    }

    // ── Deserialize ───────────────────────────────────────────────────────

    /// <summary>Deserializes a Snowed-encoded byte array to <typeparamref name="T"/>.</summary>
    public static T? Deserialize<T>(byte[] bytes, SnowedSerializerOptions? options = null)
    {
        var opts = options ?? SnowedSerializerOptions.Default;
        var snowedValue = new SnowedReader(bytes, opts.MaxDepth).ReadRoot();
        return (T?)new ReflectionSerializer(opts).Deserialize(snowedValue, typeof(T));
    }

    /// <summary>Deserializes from a stream synchronously by buffering the entire stream.</summary>
    public static T? Deserialize<T>(Stream stream, SnowedSerializerOptions? options = null)
    {
        using var ms = new MemoryStream();
        stream.CopyTo(ms);
        return Deserialize<T>(ms.ToArray(), options);
    }

    /// <summary>Deserializes from a stream asynchronously.</summary>
    public static async Task<T?> DeserializeAsync<T>(Stream stream,
        SnowedSerializerOptions? options = null, CancellationToken cancellationToken = default)
    {
        using var ms = new MemoryStream();
        await stream.CopyToAsync(ms, cancellationToken).ConfigureAwait(false);
        return Deserialize<T>(ms.ToArray(), options);
    }

    // ── Untyped ───────────────────────────────────────────────────────────

    /// <summary>Parses Snowed bytes into a <see cref="SnowedValue"/> without a target type.</summary>
    public static SnowedValue DeserializeValue(byte[] bytes, SnowedSerializerOptions? options = null)
    {
        var opts = options ?? SnowedSerializerOptions.Default;
        return new SnowedReader(bytes, opts.MaxDepth).ReadRoot();
    }

    /// <summary>Parses a Snowed stream into a <see cref="SnowedValue"/> asynchronously.</summary>
    public static async Task<SnowedValue> DeserializeValueAsync(Stream stream,
        SnowedSerializerOptions? options = null, CancellationToken cancellationToken = default)
    {
        using var ms = new MemoryStream();
        await stream.CopyToAsync(ms, cancellationToken).ConfigureAwait(false);
        return DeserializeValue(ms.ToArray(), options);
    }

    // ── Encode / Decode SnowedValue directly ──────────────────────────────

    /// <summary>Encodes a <see cref="SnowedValue"/> to bytes without going through the type system.</summary>
    public static byte[] Encode(SnowedValue value)
    {
        var ms = new MemoryStream();
        new SnowedWriter(ms).Write(value);
        return ms.ToArray();
    }

    // ── Internal helpers ──────────────────────────────────────────────────

    private static void SerializeToStream(Stream stream, object? value, Type type,
        SnowedSerializerOptions? options)
    {
        var opts = options ?? SnowedSerializerOptions.Default;
        var snowedValue = new ReflectionSerializer(opts).Serialize(value);
        new SnowedWriter(stream).Write(snowedValue);
    }
}
