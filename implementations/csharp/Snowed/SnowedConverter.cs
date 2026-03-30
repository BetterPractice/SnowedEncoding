namespace Snowed;

/// <summary>Non-generic base class for all Snowed converters.</summary>
public abstract class SnowedConverter
{
    /// <summary>Returns true if this converter handles the given type.</summary>
    public abstract bool CanConvert(Type typeToConvert);

    internal abstract SnowedValue WriteCore(object? value, SnowedSerializerOptions options);
    internal abstract object? ReadCore(SnowedValue value, SnowedSerializerOptions options);
}

/// <summary>
/// Override this to provide custom serialization for type <typeparamref name="T"/>.
/// Register instances via <see cref="SnowedSerializerOptions.Converters"/>.
/// </summary>
/// <example>
/// <code>
/// public class IpAddressConverter : SnowedConverter&lt;IPAddress&gt;
/// {
///     public override SnowedValue Write(IPAddress? value, SnowedSerializerOptions options)
///         => value is null ? SnowedNull.Instance : SnowedValue.Create(value.ToString());
///
///     public override IPAddress? Read(SnowedValue value, SnowedSerializerOptions options)
///         => value is SnowedString s ? IPAddress.Parse(s.Value) : null;
/// }
/// </code>
/// </example>
public abstract class SnowedConverter<T> : SnowedConverter
{
    public override bool CanConvert(Type typeToConvert)
        => typeToConvert == typeof(T);

    /// <summary>Converts <paramref name="value"/> to a <see cref="SnowedValue"/>.</summary>
    public abstract SnowedValue Write(T? value, SnowedSerializerOptions options);

    /// <summary>Converts a <see cref="SnowedValue"/> to <typeparamref name="T"/>.</summary>
    public abstract T? Read(SnowedValue value, SnowedSerializerOptions options);

    internal override SnowedValue WriteCore(object? value, SnowedSerializerOptions options)
        => Write(value is null ? default : (T)value, options);

    internal override object? ReadCore(SnowedValue value, SnowedSerializerOptions options)
        => Read(value, options);
}
