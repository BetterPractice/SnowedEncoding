namespace Snowed;

/// <summary>
/// Determines how property names are converted to Snowed object keys.
/// </summary>
public abstract class SnowedNamingPolicy
{
    /// <summary>
    /// Converts <c>PascalCase</c> property names to <c>camelCase</c> keys.
    /// This is the most common convention for JSON-style APIs.
    /// </summary>
    public static SnowedNamingPolicy CamelCase { get; } = new CamelCasePolicy();

    /// <summary>Converts a property name to the key name used in the encoded object.</summary>
    public abstract string ConvertName(string name);

    private sealed class CamelCasePolicy : SnowedNamingPolicy
    {
        public override string ConvertName(string name)
        {
            if (string.IsNullOrEmpty(name) || !char.IsUpper(name[0]))
                return name;
            return char.ToLowerInvariant(name[0]) + name[1..];
        }
    }
}
