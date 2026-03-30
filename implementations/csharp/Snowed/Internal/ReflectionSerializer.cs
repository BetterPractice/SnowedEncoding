using System.Collections;
using System.Reflection;

namespace Snowed.Internal;

/// <summary>
/// Converts between CLR objects and <see cref="SnowedValue"/> trees using reflection.
/// This is the v1 implementation. A source-generation path is planned for a future version.
/// </summary>
internal sealed class ReflectionSerializer
{
    private readonly SnowedSerializerOptions _options;

    internal ReflectionSerializer(SnowedSerializerOptions options) { _options = options; }

    // ── Serialization ─────────────────────────────────────────────────────

    internal SnowedValue Serialize(object? value, int depth = 0)
    {
        if (depth > _options.MaxDepth)
            throw new SnowedException($"Maximum serialization depth {_options.MaxDepth} exceeded");

        if (value is null) return SnowedNull.Instance;

        Type type = value.GetType();

        // Custom converter
        var converter = _options.FindConverter(type);
        if (converter is not null) return converter.WriteCore(value, _options);

        // Built-in types
        return value switch
        {
            bool b             => SnowedValue.Create(b),
            sbyte n            => SnowedValue.Create((long)n),
            short n            => SnowedValue.Create((long)n),
            int n              => SnowedValue.Create((long)n),
            long n             => SnowedValue.Create(n),
            byte n             => SnowedValue.Create((ulong)n),
            ushort n           => SnowedValue.Create((ulong)n),
            uint n             => SnowedValue.Create((ulong)n),
            ulong n            => SnowedValue.Create(n),
            float f            => SnowedValue.Create((double)f),
            double f           => SnowedValue.Create(f),
            decimal d          => SnowedValue.Create(d),
            DateTime dt        => SnowedValue.Create(new DateTimeOffset(dt, TimeSpan.Zero)),
            DateTimeOffset dto => SnowedValue.Create(dto),
            string s           => SnowedValue.Create(s),
            byte[] bytes       => SnowedValue.Create(bytes),
            Memory<byte> m     => SnowedValue.Create(m),
            ReadOnlyMemory<byte> m => SnowedValue.Create(m),
            Guid g             => SnowedValue.Create(g),
            _                  => SerializeComplex(value, type, depth)
        };
    }

    private SnowedValue SerializeComplex(object value, Type type, int depth)
    {
        // Nullable<T> is unwrapped before this point (null check at top handles null)

        // Dictionary<string, TValue>
        if (TryGetStringDictionary(value, type, out var dictItems))
        {
            var obj = SnowedValue.CreateObject();
            foreach (var (k, v) in dictItems!)
                obj[k] = Serialize(v, depth + 1);
            return obj;
        }

        // IEnumerable (non-string, non-dictionary)
        if (value is IEnumerable enumerable && value is not string)
        {
            var arr = new SnowedArray();
            foreach (var item in enumerable)
                arr.Add(Serialize(item, depth + 1));
            return arr;
        }

        // POCO — serialize public instance properties
        return SerializePoco(value, type, depth);
    }

    private SnowedValue SerializePoco(object value, Type type, int depth)
    {
        var obj = SnowedValue.CreateObject();
        foreach (var prop in GetProperties(type))
        {
            if (prop.GetCustomAttribute<SnowedIgnoreAttribute>() is not null) continue;
            string key = GetKeyName(prop);
            object? propValue = prop.GetValue(value);
            obj[key] = Serialize(propValue, depth + 1);
        }
        return obj;
    }

    // ── Deserialization ───────────────────────────────────────────────────

    internal object? Deserialize(SnowedValue value, Type targetType, int depth = 0)
    {
        if (depth > _options.MaxDepth)
            throw new SnowedException($"Maximum deserialization depth {_options.MaxDepth} exceeded");

        // Custom converter
        var converter = _options.FindConverter(targetType);
        if (converter is not null) return converter.ReadCore(value, _options);

        // Null
        if (value is SnowedNull)
        {
            if (targetType.IsValueType && Nullable.GetUnderlyingType(targetType) is null)
                throw new SnowedTypeMismatchException(typeof(SnowedNull), targetType, "target is a non-nullable value type");
            return null;
        }

        // Unwrap Nullable<T>
        Type underlying = Nullable.GetUnderlyingType(targetType) ?? targetType;

        // SnowedValue passthrough
        if (underlying == typeof(SnowedValue) || underlying.IsAssignableTo(typeof(SnowedValue)))
            return value;

        return underlying switch
        {
            _ when underlying == typeof(bool)              => CoerceToBool(value),
            _ when underlying == typeof(sbyte)             => (sbyte)CoerceToInt64(value, underlying),
            _ when underlying == typeof(short)             => (short)CoerceToInt64(value, underlying),
            _ when underlying == typeof(int)               => (int)CoerceToInt64(value, underlying),
            _ when underlying == typeof(long)              => CoerceToInt64(value, underlying),
            _ when underlying == typeof(byte)              => (byte)CoerceToUInt64(value, underlying),
            _ when underlying == typeof(ushort)            => (ushort)CoerceToUInt64(value, underlying),
            _ when underlying == typeof(uint)              => (uint)CoerceToUInt64(value, underlying),
            _ when underlying == typeof(ulong)             => CoerceToUInt64(value, underlying),
            _ when underlying == typeof(float)             => (float)CoerceToFloat64(value),
            _ when underlying == typeof(double)            => CoerceToFloat64(value),
            _ when underlying == typeof(decimal)           => CoerceToDecimal(value),
            _ when underlying == typeof(DateTime)          => CoerceToDateTime(value).UtcDateTime,
            _ when underlying == typeof(DateTimeOffset)    => CoerceToDateTime(value),
            _ when underlying == typeof(string)            => CoerceToString(value),
            _ when underlying == typeof(byte[])            => CoerceToBinary(value),
            _ when underlying == typeof(Guid)              => CoerceToGuid(value),
            _ => DeserializeComplex(value, underlying, depth)
        };
    }

    private object DeserializeComplex(SnowedValue value, Type type, int depth)
    {
        // Array types
        if (type.IsArray)
        {
            var arr = RequireArray(value);
            Type elem = type.GetElementType()!;
            Array result = Array.CreateInstance(elem, arr.Count);
            for (int i = 0; i < arr.Count; i++)
                result.SetValue(Deserialize(arr[i], elem, depth + 1), i);
            return result;
        }

        // List<T> or IList<T>
        if (type.IsGenericType)
        {
            Type genDef = type.GetGenericTypeDefinition();
            Type[] args = type.GetGenericArguments();

            if (genDef == typeof(List<>) || genDef == typeof(IList<>) || genDef == typeof(ICollection<>) ||
                genDef == typeof(IEnumerable<>))
            {
                var arr = RequireArray(value);
                Type listType = typeof(List<>).MakeGenericType(args[0]);
                var list = (IList)Activator.CreateInstance(listType)!;
                foreach (var item in arr.Items)
                    list.Add(Deserialize(item, args[0], depth + 1));
                return list;
            }

            if (genDef == typeof(Dictionary<,>) || genDef == typeof(IDictionary<,>))
            {
                if (args[0] != typeof(string))
                    throw new SnowedException("Only Dictionary<string, T> is supported");
                var obj = RequireObject(value);
                Type dictType = typeof(Dictionary<,>).MakeGenericType(args[0], args[1]);
                var dict = (IDictionary)Activator.CreateInstance(dictType)!;
                foreach (var kv in obj.Pairs)
                    dict[kv.Key] = Deserialize(kv.Value, args[1], depth + 1)!;
                return dict;
            }
        }

        // POCO
        return DeserializePoco(value, type, depth);
    }

    private object DeserializePoco(SnowedValue value, Type type, int depth)
    {
        var obj = RequireObject(value);
        object instance = Activator.CreateInstance(type)
            ?? throw new SnowedException($"Cannot create instance of {type.FullName}: no parameterless constructor");

        // Build a lookup of key → property
        var propByKey = new Dictionary<string, PropertyInfo>(StringComparer.OrdinalIgnoreCase);
        foreach (var prop in GetProperties(type))
        {
            if (prop.GetCustomAttribute<SnowedIgnoreAttribute>() is not null) continue;
            if (prop.GetSetMethod() is null) continue;
            propByKey[GetKeyName(prop)] = prop;
        }

        foreach (var kv in obj.Pairs)
        {
            if (propByKey.TryGetValue(kv.Key, out var prop))
            {
                object? propVal = Deserialize(kv.Value, prop.PropertyType, depth + 1);
                prop.SetValue(instance, propVal);
            }
            else if (_options.UnknownFieldHandling == UnknownFieldHandling.Error)
            {
                throw new SnowedException($"Unknown field '{kv.Key}' on type {type.Name}");
            }
        }

        return instance;
    }

    // ── Coercion helpers ──────────────────────────────────────────────────

    private static bool CoerceToBool(SnowedValue v)
    {
        if (v is SnowedBoolean b) return b.Value;
        throw Mismatch(v, typeof(bool));
    }

    private static long CoerceToInt64(SnowedValue v, Type target)
    {
        long result = v switch
        {
            SnowedInt64 i  => i.Value,
            SnowedUInt64 u => checked((long)u.Value),
            SnowedFloat64 f when !f.Value.IsNaN() && !f.Value.IsInfinity() &&
                                  f.Value == Math.Truncate(f.Value) => (long)f.Value,
            _ => throw Mismatch(v, target)
        };
        // Range check for smaller integer types
        return target switch
        {
            _ when target == typeof(sbyte) => result is >= sbyte.MinValue  and <= sbyte.MaxValue  ? result : throw Overflow(result, target),
            _ when target == typeof(short) => result is >= short.MinValue  and <= short.MaxValue  ? result : throw Overflow(result, target),
            _ when target == typeof(int)   => result is >= int.MinValue    and <= int.MaxValue    ? result : throw Overflow(result, target),
            _                              => result
        };
    }

    private static ulong CoerceToUInt64(SnowedValue v, Type target)
    {
        ulong result = v switch
        {
            SnowedUInt64 u => u.Value,
            SnowedInt64 i  => i.Value >= 0 ? (ulong)i.Value : throw Overflow(i.Value, target),
            SnowedFloat64 f when !f.Value.IsNaN() && !f.Value.IsInfinity() && f.Value >= 0 &&
                                  f.Value == Math.Truncate(f.Value) => (ulong)f.Value,
            _ => throw Mismatch(v, target)
        };
        return target switch
        {
            _ when target == typeof(byte)   => result <= byte.MaxValue   ? result : throw Overflow(result, target),
            _ when target == typeof(ushort) => result <= ushort.MaxValue ? result : throw Overflow(result, target),
            _ when target == typeof(uint)   => result <= uint.MaxValue   ? result : throw Overflow(result, target),
            _                               => result
        };
    }

    private static double CoerceToFloat64(SnowedValue v) => v switch
    {
        SnowedFloat64 f => f.Value,
        SnowedInt64 i   => (double)i.Value,
        SnowedUInt64 u  => (double)u.Value,
        _               => throw Mismatch(v, typeof(double))
    };

    private static decimal CoerceToDecimal(SnowedValue v) => v switch
    {
        SnowedDecimal d => d.Value,
        SnowedInt64 i   => (decimal)i.Value,
        SnowedUInt64 u  => (decimal)u.Value,
        _               => throw Mismatch(v, typeof(decimal))
    };

    private static DateTimeOffset CoerceToDateTime(SnowedValue v)
    {
        if (v is SnowedDateTime dt) return dt.Value;
        throw Mismatch(v, typeof(DateTimeOffset));
    }

    private static string CoerceToString(SnowedValue v)
    {
        if (v is SnowedString s) return s.Value;
        throw Mismatch(v, typeof(string));
    }

    private static byte[] CoerceToBinary(SnowedValue v)
    {
        if (v is SnowedBinary b) return b.Value.ToArray();
        throw Mismatch(v, typeof(byte[]));
    }

    private static Guid CoerceToGuid(SnowedValue v)
    {
        if (v is SnowedUuid u) return u.Value;
        throw Mismatch(v, typeof(Guid));
    }

    private static SnowedArray RequireArray(SnowedValue v)
    {
        if (v is SnowedArray a) return a;
        throw Mismatch(v, typeof(IEnumerable<object>));
    }

    private static SnowedObject RequireObject(SnowedValue v)
    {
        if (v is SnowedObject o) return o;
        throw Mismatch(v, typeof(object));
    }

    // ── Property helpers ──────────────────────────────────────────────────

    private static IEnumerable<PropertyInfo> GetProperties(Type type)
        => type.GetProperties(BindingFlags.Instance | BindingFlags.Public)
               .Where(p => p.CanRead);

    private string GetKeyName(PropertyInfo prop)
    {
        var attr = prop.GetCustomAttribute<SnowedPropertyNameAttribute>();
        if (attr is not null) return attr.Name;
        return _options.PropertyNamingPolicy?.ConvertName(prop.Name) ?? prop.Name;
    }

    // ── Dictionary detection ──────────────────────────────────────────────

    private static bool TryGetStringDictionary(object value, Type type,
        out IEnumerable<(string Key, object? Value)>? items)
    {
        // Look for IDictionary<string, TValue>
        foreach (var iface in type.GetInterfaces())
        {
            if (!iface.IsGenericType) continue;
            var genDef = iface.GetGenericTypeDefinition();
            if (genDef != typeof(IDictionary<,>)) continue;
            var args = iface.GetGenericArguments();
            if (args[0] != typeof(string)) continue;

            var dict = (IDictionary)value;
            items = dict.Keys.Cast<string>()
                        .Select(k => (k, (object?)dict[k]));
            return true;
        }
        items = null;
        return false;
    }

    // ── Error factories ───────────────────────────────────────────────────

    private static SnowedTypeMismatchException Mismatch(SnowedValue v, Type target)
        => new(v.GetType(), target);

    private static SnowedException Overflow(object raw, Type target)
        => new($"Value {raw} overflows target type {target.Name}");
}

// Small extension to avoid System.Double noise in calling code
file static class DoubleExtensions
{
    internal static bool IsNaN(this double d)       => double.IsNaN(d);
    internal static bool IsInfinity(this double d)  => double.IsInfinity(d);
}
