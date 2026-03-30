namespace Snowed;

/// <summary>
/// Overrides the key name used when serializing or deserializing a property.
/// </summary>
[AttributeUsage(AttributeTargets.Property | AttributeTargets.Field, AllowMultiple = false)]
public sealed class SnowedPropertyNameAttribute : Attribute
{
    public string Name { get; }
    public SnowedPropertyNameAttribute(string name) { Name = name; }
}

/// <summary>
/// Excludes a property or field from Snowed serialization and deserialization.
/// </summary>
[AttributeUsage(AttributeTargets.Property | AttributeTargets.Field, AllowMultiple = false)]
public sealed class SnowedIgnoreAttribute : Attribute { }
