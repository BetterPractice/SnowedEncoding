namespace Snowed;

/// <summary>Thrown when a Snowed encoding or decoding operation fails.</summary>
public class SnowedException : Exception
{
    public SnowedException(string message) : base(message) { }
    public SnowedException(string message, Exception inner) : base(message, inner) { }
}

/// <summary>
/// Thrown when a Snowed value cannot be converted to the requested target type.
/// </summary>
public sealed class SnowedTypeMismatchException : SnowedException
{
    public Type SourceType { get; }
    public Type TargetType { get; }

    public SnowedTypeMismatchException(Type sourceType, Type targetType, string? detail = null)
        : base($"Cannot convert Snowed {sourceType.Name} to {targetType.Name}" +
               (detail is null ? "" : $": {detail}"))
    {
        SourceType = sourceType;
        TargetType = targetType;
    }
}
