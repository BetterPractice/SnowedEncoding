namespace Snowed;

/// <summary>How to handle keys in the encoded document that have no matching property on the target type.</summary>
public enum UnknownFieldHandling
{
    /// <summary>Silently skip unrecognised keys (default).</summary>
    Ignore,
    /// <summary>Throw a <see cref="SnowedException"/> when an unrecognised key is encountered.</summary>
    Error,
}

/// <summary>How to handle duplicate keys within an encoded Object.</summary>
public enum DuplicateKeyHandling
{
    /// <summary>The first occurrence wins; subsequent duplicate keys are ignored (default).</summary>
    FirstWins,
    /// <summary>The last occurrence wins; earlier values are overwritten.</summary>
    LastWins,
    /// <summary>Throw a <see cref="SnowedException"/> when a duplicate key is encountered.</summary>
    Error,
}

/// <summary>
/// Controls the behaviour of <see cref="SnowedSerializer"/>.
/// </summary>
public sealed class SnowedSerializerOptions
{
    /// <summary>Custom converters consulted before built-in type handling. Evaluated in list order.</summary>
    public IList<SnowedConverter> Converters { get; } = new List<SnowedConverter>();

    /// <summary>
    /// Policy applied to property names when producing object keys.
    /// <c>null</c> means use the property name as-is.
    /// A <see cref="SnowedPropertyNameAttribute"/> on a property always takes precedence.
    /// </summary>
    public SnowedNamingPolicy? PropertyNamingPolicy { get; set; }

    /// <summary>How to handle object keys in encoded data that have no matching target property.</summary>
    public UnknownFieldHandling UnknownFieldHandling { get; set; } = UnknownFieldHandling.Ignore;

    /// <summary>How to handle duplicate object keys encountered during decoding.</summary>
    public DuplicateKeyHandling DuplicateKeyHandling { get; set; } = DuplicateKeyHandling.LastWins;

    /// <summary>
    /// Maximum nesting depth before a <see cref="SnowedException"/> is thrown.
    /// Guards against stack overflows from pathologically nested documents.
    /// </summary>
    public int MaxDepth { get; set; } = 64;

    /// <summary>A shared default instance with no customisation.</summary>
    public static SnowedSerializerOptions Default { get; } = new SnowedSerializerOptions();

    /// <summary>Finds the first converter that handles <paramref name="type"/>, or <c>null</c>.</summary>
    internal SnowedConverter? FindConverter(Type type)
    {
        foreach (var c in Converters)
            if (c.CanConvert(type)) return c;
        return null;
    }
}
