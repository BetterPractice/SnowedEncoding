using Snowed;
using Xunit;

namespace Snowed.Tests;

public class RoundTripTests
{
    // ── Null ──────────────────────────────────────────────────────────────

    [Fact] public void Null_RoundTrips()
    {
        var rt = Decode(SnowedNull.Instance);
        Assert.IsType<SnowedNull>(rt);
    }

    // ── Boolean ───────────────────────────────────────────────────────────

    [Fact] public void BoolFalse_RoundTrips() => AssertRoundTrip(SnowedValue.Create(false));
    [Fact] public void BoolTrue_RoundTrips()  => AssertRoundTrip(SnowedValue.Create(true));

    // ── Int64 ─────────────────────────────────────────────────────────────

    [Theory]
    [InlineData(0L)]
    [InlineData(42L)]
    [InlineData(-1L)]
    [InlineData(long.MinValue)]
    [InlineData(long.MaxValue)]
    public void Int64_RoundTrips(long value) =>
        AssertRoundTrip(SnowedValue.Create(value));

    // ── UInt64 ────────────────────────────────────────────────────────────

    [Theory]
    [InlineData(0UL)]
    [InlineData(ulong.MaxValue)]
    public void UInt64_RoundTrips(ulong value) =>
        AssertRoundTrip(SnowedValue.Create(value));

    [Fact] public void UInt64_AboveInt64Max_RoundTrips() =>
        AssertRoundTrip(SnowedValue.Create((ulong)long.MaxValue + 1));

    // ── Float64 ───────────────────────────────────────────────────────────

    [Fact] public void Float64_RoundTrips()          => AssertRoundTrip(SnowedValue.Create(3.14));
    [Fact] public void Float64_Infinity_RoundTrips() => AssertRoundTrip(SnowedValue.Create(double.PositiveInfinity));
    [Fact] public void Float64_NegInfinity_RoundTrips() => AssertRoundTrip(SnowedValue.Create(double.NegativeInfinity));

    [Fact]
    public void Float64_NaN_RoundTrips()
    {
        var bytes = SnowedSerializer.Encode(SnowedValue.Create(double.NaN));
        var rt = (SnowedFloat64)SnowedSerializer.DeserializeValue(bytes);
        Assert.True(double.IsNaN(rt.Value));
    }

    [Fact]
    public void Float64_NegativeZero_RoundTrips()
    {
        var bytes = SnowedSerializer.Encode(SnowedValue.Create(-0.0));
        var rt = (SnowedFloat64)SnowedSerializer.DeserializeValue(bytes);
        // -0.0 and 0.0 compare equal but have different bit patterns
        Assert.True(double.IsNegativeInfinity(1.0 / rt.Value));
    }

    // ── DateTime ─────────────────────────────────────────────────────────

    [Fact]
    public void DateTime_RoundTrips()
    {
        var dt = new DateTimeOffset(2024, 1, 15, 12, 0, 0, TimeSpan.Zero);
        var rt = (SnowedDateTime)Decode(SnowedValue.Create(dt));
        Assert.Equal(dt, rt.Value, TimeSpan.FromMilliseconds(1));
    }

    [Fact]
    public void DateTime_Epoch_RoundTrips()
    {
        var epoch = DateTimeOffset.UnixEpoch;
        var rt = (SnowedDateTime)Decode(SnowedValue.Create(epoch));
        Assert.Equal(epoch, rt.Value, TimeSpan.FromMilliseconds(1));
    }

    [Fact]
    public void DateTime_PreEpoch_RoundTrips()
    {
        var dt = new DateTimeOffset(1960, 1, 1, 0, 0, 0, TimeSpan.Zero);
        var rt = (SnowedDateTime)Decode(SnowedValue.Create(dt));
        Assert.Equal(dt, rt.Value, TimeSpan.FromMilliseconds(1));
    }

    [Fact]
    public void DateTime_NonUTC_NormalisedToUTC()
    {
        var dt = new DateTimeOffset(2024, 1, 1, 12, 0, 0, TimeSpan.FromHours(5));
        var rt = (SnowedDateTime)Decode(SnowedValue.Create(dt));
        Assert.Equal(dt.ToUniversalTime(), rt.Value, TimeSpan.FromMilliseconds(1));
    }

    // ── Binary ────────────────────────────────────────────────────────────

    [Fact] public void Binary_Empty_RoundTrips()   => AssertRoundTrip(SnowedValue.Create(Array.Empty<byte>()));
    [Fact] public void Binary_Payload_RoundTrips() => AssertRoundTrip(SnowedValue.Create(new byte[] { 0xDE, 0xAD, 0xBE, 0xEF }));

    // ── String ───────────────────────────────────────────────────────────

    [Fact] public void String_Empty_RoundTrips()   => AssertRoundTrip(SnowedValue.Create(string.Empty));
    [Fact] public void String_ASCII_RoundTrips()   => AssertRoundTrip(SnowedValue.Create("hello"));
    [Fact] public void String_Unicode_RoundTrips() => AssertRoundTrip(SnowedValue.Create("Héllo wörld 🌍"));

    // ── Decimal ──────────────────────────────────────────────────────────

    [Fact] public void Decimal_Integer_RoundTrips()    => AssertRoundTrip(SnowedValue.Create(42m));
    [Fact] public void Decimal_Fractional_RoundTrips() => AssertRoundTrip(SnowedValue.Create(3.14159m));
    [Fact] public void Decimal_Negative_RoundTrips()   => AssertRoundTrip(SnowedValue.Create(-99.99m));

    [Fact]
    public void Decimal_ScaleNotPreserved()
    {
        // 1.00 and 1.0 and 1 all encode to "1"
        var bytes = SnowedSerializer.Encode(SnowedValue.Create(1.00m));
        var rt = (SnowedDecimal)SnowedSerializer.DeserializeValue(bytes);
        Assert.Equal(1m, rt.Value);
    }

    // ── UUID ─────────────────────────────────────────────────────────────

    [Fact]
    public void Uuid_RoundTrips()
    {
        var guid = Guid.NewGuid();
        AssertRoundTrip(SnowedValue.Create(guid));
    }

    [Fact]
    public void Uuid_KnownWireFormat()
    {
        // 550e8400-e29b-41d4-a716-446655440000 in RFC 4122 byte order
        var expected = new byte[]
        {
            0x13,
            0x10, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,  // length = 16
            0x55, 0x0e, 0x84, 0x00, 0xe2, 0x9b, 0x41, 0xd4,
            0xa7, 0x16, 0x44, 0x66, 0x55, 0x44, 0x00, 0x00
        };
        var guid = Guid.Parse("550e8400-e29b-41d4-a716-446655440000");
        Assert.Equal(expected, SnowedSerializer.Encode(SnowedValue.Create(guid)));
    }

    [Fact]
    public void Uuid_DecodesFromRfc4122Bytes()
    {
        var bytes = new byte[]
        {
            0x13,
            0x10, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x55, 0x0e, 0x84, 0x00, 0xe2, 0x9b, 0x41, 0xd4,
            0xa7, 0x16, 0x44, 0x66, 0x55, 0x44, 0x00, 0x00
        };
        var rt = (SnowedUuid)SnowedSerializer.DeserializeValue(bytes);
        Assert.Equal(Guid.Parse("550e8400-e29b-41d4-a716-446655440000"), rt.Value);
    }

    [Fact]
    public void Uuid_InvalidLengthThrows()
    {
        var bytes = new byte[]
        {
            0x13,
            0x04, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,  // length = 4, wrong
            0x01, 0x02, 0x03, 0x04
        };
        Assert.Throws<SnowedException>(() => SnowedSerializer.DeserializeValue(bytes));
    }

    // ── Array ─────────────────────────────────────────────────────────────

    [Fact] public void Array_Empty_RoundTrips() => AssertRoundTrip(new SnowedArray());

    [Fact]
    public void Array_Mixed_RoundTrips()
    {
        var arr = new SnowedArray();
        arr.Add(SnowedNull.Instance);
        arr.Add(SnowedValue.Create(true));
        arr.Add(SnowedValue.Create(7L));
        arr.Add(SnowedValue.Create("x"));
        AssertRoundTrip(arr);
    }

    [Fact]
    public void Array_Nested_RoundTrips()
    {
        var inner = new SnowedArray();
        inner.Add(SnowedValue.Create(1L));
        inner.Add(SnowedValue.Create(2L));
        var outer = new SnowedArray();
        outer.Add(inner);
        outer.Add(new SnowedArray());
        AssertRoundTrip(outer);
    }

    // ── Object ────────────────────────────────────────────────────────────

    [Fact] public void Object_Empty_RoundTrips() => AssertRoundTrip(new SnowedObject());

    [Fact]
    public void Object_Simple_RoundTrips()
    {
        var obj = SnowedValue.CreateObject();
        obj["name"] = SnowedValue.Create("Alice");
        obj["age"]  = SnowedValue.Create(30L);
        AssertRoundTrip(obj);
    }

    [Fact]
    public void Object_MaxLengthKey_RoundTrips()
    {
        var obj = SnowedValue.CreateObject();
        obj[new string('k', 255)] = SnowedNull.Instance;
        AssertRoundTrip(obj);
    }

    [Fact]
    public void Object_KeyTooLong_Throws()
    {
        var obj = SnowedValue.CreateObject();
        obj[new string('k', 256)] = SnowedNull.Instance;
        Assert.Throws<SnowedException>(() => SnowedSerializer.Encode(obj));
    }

    // ── POCO Codable round-trips ──────────────────────────────────────────

    public class SimplePoco
    {
        public string Name    { get; set; } = "";
        public int Count      { get; set; }
        public double Ratio   { get; set; }
        public bool Active    { get; set; }
    }

    [Fact]
    public void Poco_Simple_RoundTrips()
    {
        var original = new SimplePoco { Name = "test", Count = 42, Ratio = 0.5, Active = true };
        var rt = Cycle<SimplePoco>(original);
        Assert.Equal(original.Name,   rt.Name);
        Assert.Equal(original.Count,  rt.Count);
        Assert.Equal(original.Ratio,  rt.Ratio);
        Assert.Equal(original.Active, rt.Active);
    }

    public class WithNullable { public string? Value { get; set; } }

    [Fact]
    public void Poco_NullableNull_RoundTrips()
    {
        var original = new WithNullable { Value = null };
        Assert.Null(Cycle<WithNullable>(original).Value);
    }

    [Fact]
    public void Poco_NullablePresent_RoundTrips()
    {
        var original = new WithNullable { Value = "hello" };
        Assert.Equal("hello", Cycle<WithNullable>(original).Value);
    }

    public class WithBytes { public byte[] Blob { get; set; } = Array.Empty<byte>(); }

    [Fact]
    public void Poco_Bytes_RoundTrips()
    {
        var original = new WithBytes { Blob = new byte[] { 1, 2, 3, 4 } };
        Assert.Equal(original.Blob, Cycle<WithBytes>(original).Blob);
    }

    public class WithDate { public DateTimeOffset Ts { get; set; } }

    [Fact]
    public void Poco_Date_RoundTrips()
    {
        var original = new WithDate { Ts = new DateTimeOffset(2024, 6, 1, 0, 0, 0, TimeSpan.Zero) };
        var rt = Cycle<WithDate>(original);
        Assert.Equal(original.Ts, rt.Ts, TimeSpan.FromMilliseconds(1));
    }

    public class WithDecimalProp { public decimal Price { get; set; } }

    [Fact]
    public void Poco_Decimal_RoundTrips()
    {
        var original = new WithDecimalProp { Price = 19.99m };
        Assert.Equal(original.Price, Cycle<WithDecimalProp>(original).Price);
    }

    public class WithGuid { public Guid Id { get; set; } }

    [Fact]
    public void Poco_Guid_RoundTrips()
    {
        var original = new WithGuid { Id = Guid.NewGuid() };
        Assert.Equal(original.Id, Cycle<WithGuid>(original).Id);
    }

    public class Nested { public Inner Child { get; set; } = new(); public string Label { get; set; } = ""; }
    public class Inner  { public int X { get; set; } }

    [Fact]
    public void Poco_Nested_RoundTrips()
    {
        var original = new Nested { Child = new Inner { X = 7 }, Label = "test" };
        var rt = Cycle<Nested>(original);
        Assert.Equal(original.Child.X, rt.Child.X);
        Assert.Equal(original.Label,   rt.Label);
    }

    [Fact]
    public void List_RoundTrips()
    {
        var original = new List<int> { 1, 2, 3 };
        Assert.Equal(original, Cycle<List<int>>(original));
    }

    [Fact]
    public void Dictionary_RoundTrips()
    {
        var original = new Dictionary<string, int> { ["a"] = 1, ["b"] = 2 };
        var rt = Cycle<Dictionary<string, int>>(original);
        Assert.Equal(original["a"], rt["a"]);
        Assert.Equal(original["b"], rt["b"]);
    }

    // ── Numeric coercion ──────────────────────────────────────────────────

    [Fact]
    public void Int64_DecodesAs_Int32()
    {
        var bytes = SnowedSerializer.Encode(SnowedValue.Create(100L));
        Assert.Equal(100, SnowedSerializer.Deserialize<int>(bytes));
    }

    [Fact]
    public void Int64_Overflow_Int32_Throws()
    {
        var bytes = SnowedSerializer.Encode(SnowedValue.Create((long)int.MaxValue + 1));
        Assert.Throws<SnowedException>(() => SnowedSerializer.Deserialize<int>(bytes));
    }

    [Fact]
    public void Float64_WholeNumber_DecodesAs_Int64()
    {
        var bytes = SnowedSerializer.Encode(SnowedValue.Create(7.0));
        Assert.Equal(7L, SnowedSerializer.Deserialize<long>(bytes));
    }

    [Fact]
    public void Float64_Fractional_Throws_WhenDecodedAs_Int64()
    {
        var bytes = SnowedSerializer.Encode(SnowedValue.Create(7.5));
        Assert.Throws<SnowedTypeMismatchException>(() => SnowedSerializer.Deserialize<long>(bytes));
    }

    // ── Wire format spot checks ───────────────────────────────────────────

    [Fact]
    public void Null_WireFormat()
    {
        Assert.Equal(new byte[] { 0x00 }, SnowedSerializer.Encode(SnowedNull.Instance));
    }

    [Fact]
    public void BoolFalse_WireFormat()
    {
        Assert.Equal(new byte[] { 0x02 }, SnowedSerializer.Encode(SnowedValue.Create(false)));
    }

    [Fact]
    public void BoolTrue_WireFormat()
    {
        Assert.Equal(new byte[] { 0x03 }, SnowedSerializer.Encode(SnowedValue.Create(true)));
    }

    [Fact]
    public void Int64_42_WireFormat()
    {
        var expected = new byte[] { 0x04, 0x2A, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00 };
        Assert.Equal(expected, SnowedSerializer.Encode(SnowedValue.Create(42L)));
    }

    [Fact]
    public void UInt64_Max_WireFormat()
    {
        var expected = new byte[] { 0x05, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF };
        Assert.Equal(expected, SnowedSerializer.Encode(SnowedValue.Create(ulong.MaxValue)));
    }

    [Fact]
    public void String_WireFormat()
    {
        var expected = new byte[]
        {
            0x11,
            0x05, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x68, 0x65, 0x6C, 0x6C, 0x6F
        };
        Assert.Equal(expected, SnowedSerializer.Encode(SnowedValue.Create("hello")));
    }

    [Fact]
    public void EmptyArray_WireFormat()
    {
        Assert.Equal(new byte[] { 0x40, 0xFF }, SnowedSerializer.Encode(new SnowedArray()));
    }

    [Fact]
    public void EmptyObject_WireFormat()
    {
        Assert.Equal(new byte[] { 0x80, 0x00 }, SnowedSerializer.Encode(new SnowedObject()));
    }

    // ── Forward compatibility ─────────────────────────────────────────────

    [Fact]
    public void UnknownNullSubtype_ParsedAsNull()
    {
        Assert.IsType<SnowedNull>(SnowedSerializer.DeserializeValue(new byte[] { 0x01 }));
    }

    [Fact]
    public void UnknownIntSubtype_ParsedAsInt64()
    {
        var bytes = new byte[9];
        bytes[0] = 0x06;
        BitConverter.TryWriteBytes(bytes.AsSpan(1), (long)99);
        if (!BitConverter.IsLittleEndian) Array.Reverse(bytes, 1, 8);
        var rt = (SnowedInt64)SnowedSerializer.DeserializeValue(bytes);
        Assert.Equal(99L, rt.Value);
    }

    [Fact]
    public void UnknownBinarySubtype_ParsedAsBinary()
    {
        // 0x14 is unknown, rounds down to Binary
        var bytes = new byte[] { 0x14, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xAB, 0xCD };
        var rt = (SnowedBinary)SnowedSerializer.DeserializeValue(bytes);
        Assert.Equal(new byte[] { 0xAB, 0xCD }, rt.Value.ToArray());
    }

    // ── Object termination ────────────────────────────────────────────────

    [Fact]
    public void Object_TerminatedByZeroLengthKey()
    {
        var bytes = new byte[]
        {
            0x80,
            0x01, 0x61,     // key "a"
            0x03,           // Boolean True
            0x00            // zero-length key terminator
        };
        var rt = (SnowedObject)SnowedSerializer.DeserializeValue(bytes);
        Assert.True(((SnowedBoolean)rt["a"]).Value);
        Assert.Equal(1, rt.Count);
    }

    [Fact]
    public void Object_TerminatedBySentinel()
    {
        // Key "b" before the Sentinel is discarded
        var bytes = new byte[]
        {
            0x80,
            0x01, 0x61,     // key "a"
            0x03,           // Boolean True
            0x01, 0x62,     // key "b" — discarded
            0xFF            // Sentinel in value position
        };
        var rt = (SnowedObject)SnowedSerializer.DeserializeValue(bytes);
        Assert.True(((SnowedBoolean)rt["a"]).Value);
        Assert.Equal(1, rt.Count);
    }

    // ── Options: naming policy ────────────────────────────────────────────

    public class CamelPoco { public string FirstName { get; set; } = ""; }

    [Fact]
    public void NamingPolicy_CamelCase_AppliedOnEncode()
    {
        var options = new SnowedSerializerOptions { PropertyNamingPolicy = SnowedNamingPolicy.CamelCase };
        var bytes   = SnowedSerializer.Serialize(new CamelPoco { FirstName = "Ada" }, options);
        var obj     = (SnowedObject)SnowedSerializer.DeserializeValue(bytes, options);
        Assert.True(obj.ContainsKey("firstName"));
        Assert.False(obj.ContainsKey("FirstName"));
    }

    [Fact]
    public void NamingPolicy_CamelCase_AppliedOnDecode()
    {
        var options = new SnowedSerializerOptions { PropertyNamingPolicy = SnowedNamingPolicy.CamelCase };
        var original = new CamelPoco { FirstName = "Ada" };
        var rt = Cycle<CamelPoco>(original, options);
        Assert.Equal("Ada", rt.FirstName);
    }

    // ── Options: custom converter ─────────────────────────────────────────

    public class VersionConverter : SnowedConverter<Version>
    {
        public override SnowedValue Write(Version? v, SnowedSerializerOptions _)
            => v is null ? SnowedNull.Instance : SnowedValue.Create(v.ToString());
        public override Version? Read(SnowedValue value, SnowedSerializerOptions _)
            => value is SnowedString s ? Version.Parse(s.Value) : null;
    }

    public class WithVersion { public Version? ApiVersion { get; set; } }

    [Fact]
    public void CustomConverter_RoundTrips()
    {
        var options = new SnowedSerializerOptions();
        options.Converters.Add(new VersionConverter());
        var original = new WithVersion { ApiVersion = new Version(2, 1, 3) };
        var rt = Cycle<WithVersion>(original, options);
        Assert.Equal(original.ApiVersion, rt.ApiVersion);
    }

    // ── Helpers ───────────────────────────────────────────────────────────

    private static SnowedValue Decode(SnowedValue value)
    {
        var bytes = SnowedSerializer.Encode(value);
        return SnowedSerializer.DeserializeValue(bytes);
    }

    private static void AssertRoundTrip(SnowedValue value)
    {
        var rt = Decode(value);
        AssertDeepEqual(value, rt);
    }

    private static T Cycle<T>(T value, SnowedSerializerOptions? options = null)
    {
        var bytes = SnowedSerializer.Serialize(value, options);
        return SnowedSerializer.Deserialize<T>(bytes, options)!;
    }

    private static void AssertDeepEqual(SnowedValue expected, SnowedValue actual)
    {
        switch (expected, actual)
        {
            case (SnowedNull,      SnowedNull):      break;
            case (SnowedBoolean a, SnowedBoolean b): Assert.Equal(a.Value, b.Value);     break;
            case (SnowedInt64 a,   SnowedInt64 b):   Assert.Equal(a.Value, b.Value);     break;
            case (SnowedUInt64 a,  SnowedUInt64 b):  Assert.Equal(a.Value, b.Value);     break;
            case (SnowedFloat64 a, SnowedFloat64 b): Assert.Equal(a.Value.ToString("R"), b.Value.ToString("R")); break;
            case (SnowedDateTime a,SnowedDateTime b): Assert.Equal(a.Value, b.Value, TimeSpan.FromMilliseconds(1)); break;
            case (SnowedBinary a,  SnowedBinary b):  Assert.Equal(a.Value.ToArray(), b.Value.ToArray()); break;
            case (SnowedString a,  SnowedString b):  Assert.Equal(a.Value, b.Value);     break;
            case (SnowedDecimal a, SnowedDecimal b):  Assert.Equal(a.Value, b.Value);     break;
            case (SnowedUuid a,    SnowedUuid b):    Assert.Equal(a.Value, b.Value);     break;
            case (SnowedArray a,   SnowedArray b):
                Assert.Equal(a.Count, b.Count);
                for (int i = 0; i < a.Count; i++) AssertDeepEqual(a[i], b[i]);
                break;
            case (SnowedObject a,  SnowedObject b):
                Assert.Equal(a.Count, b.Count);
                foreach (var key in a.Keys)
                {
                    Assert.True(b.ContainsKey(key), $"Missing key '{key}'");
                    AssertDeepEqual(a[key], b[key]);
                }
                break;
            default:
                Assert.Fail($"Type mismatch: expected {expected.GetType().Name}, got {actual.GetType().Name}");
                break;
        }
    }
}
