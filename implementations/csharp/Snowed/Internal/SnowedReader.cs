using System.Buffers.Binary;
using System.Text;

namespace Snowed.Internal;

/// <summary>Parses a single Snowed value from a byte array.</summary>
internal sealed class SnowedReader
{
    private readonly byte[] _data;
    private int _pos;
    private readonly int _maxDepth;
    private int _depth;

    internal SnowedReader(byte[] data, int maxDepth = 64)
    {
        _data    = data;
        _pos     = 0;
        _maxDepth = maxDepth;
        _depth   = 0;
    }

    internal SnowedValue ReadRoot() => ReadValue();

    // ── Core parser ───────────────────────────────────────────────────────

    private SnowedValue ReadValue()
    {
        byte typeByte = ReadByte();

        // Null family (0x00–0x03)
        if (typeByte <= 0x01) return SnowedNull.Instance;
        if (typeByte == 0x02) return SnowedValue.Create(false);
        if (typeByte == 0x03) return SnowedValue.Create(true);

        // Int64 family (0x04–0x07)
        if (typeByte <= 0x07)
        {
            ulong raw = ReadLE64();
            if (typeByte == 0x05) return SnowedValue.Create(raw);     // UInt64
            return SnowedValue.Create((long)raw);                      // Int64 (incl. unknown subtypes)
        }

        // Float64 family (0x08–0x0F)
        if (typeByte <= 0x0F)
        {
            double f = BitConverter.UInt64BitsToDouble(ReadLE64());
            if (typeByte == 0x09)                                      // DateTime
                return SnowedValue.Create(DateTimeOffset.UnixEpoch.AddMilliseconds(f));
            return SnowedValue.Create(f);                              // Float64 (incl. unknown)
        }

        // Binary family (0x10–0x3F)
        if (typeByte <= 0x3F)
        {
            ulong len = ReadLE64();
            if (len > int.MaxValue) throw Corrupted($"Binary length {len} exceeds platform maximum");
            byte[] payload = ReadBytes((int)len);
            return typeByte switch
            {
                0x11 => ParseString(payload),
                0x12 => ParseDecimal(payload),
                0x13 => SnowedUuid.FromRfc4122Bytes(payload),
                _    => SnowedValue.Create(payload),                   // Binary (incl. unknown)
            };
        }

        // Array family (0x40–0x7F)
        if (typeByte <= 0x7F) return ReadArray();

        // Sentinel (0xFF)
        if (typeByte == 0xFF) throw Corrupted("Unexpected Sentinel (0xFF) in value position");

        // Object family (0x80–0xFE)
        return ReadObject();
    }

    private SnowedValue ParseString(byte[] utf8)
    {
        try { return SnowedValue.Create(Encoding.UTF8.GetString(utf8)); }
        catch (DecoderFallbackException ex) { throw Corrupted("Invalid UTF-8 in String value", ex); }
    }

    private SnowedValue ParseDecimal(byte[] utf8)
    {
        string s;
        try { s = Encoding.UTF8.GetString(utf8); }
        catch (DecoderFallbackException ex) { throw Corrupted("Invalid UTF-8 in Decimal value", ex); }
        if (!decimal.TryParse(s, System.Globalization.NumberStyles.Any,
                              System.Globalization.CultureInfo.InvariantCulture, out decimal d))
            throw Corrupted($"Invalid Decimal string: '{s}'");
        return SnowedValue.Create(d);
    }

    private SnowedArray ReadArray()
    {
        EnterDepth();
        var arr = new SnowedArray();
        while (true)
        {
            byte next = ReadByte();
            if (next == 0xFF) break;                                    // Sentinel
            arr.Add(ReadValueWithTypeByte(next));
        }
        ExitDepth();
        return arr;
    }

    private SnowedObject ReadObject()
    {
        EnterDepth();
        var obj = new SnowedObject();
        while (true)
        {
            byte keyLen = ReadByte();
            if (keyLen == 0x00) break;                                  // zero-length key
            byte[] keyBytes = ReadBytes(keyLen);
            string key;
            try { key = Encoding.UTF8.GetString(keyBytes); }
            catch (DecoderFallbackException ex) { throw Corrupted("Invalid UTF-8 in object key", ex); }

            byte valueTypeByte = ReadByte();
            if (valueTypeByte == 0xFF) break;                           // Sentinel in value position

            SnowedValue val = ReadValueWithTypeByte(valueTypeByte);
            obj[key] = val;
        }
        ExitDepth();
        return obj;
    }

    // Re-enter the value parser with an already-consumed type byte
    private SnowedValue ReadValueWithTypeByte(byte typeByte)
    {
        int saved = _pos;
        // We need to push the type byte back — we can't, so inline the dispatch here.
        // Duplicate of ReadValue() body but without the leading ReadByte() call.
        if (typeByte <= 0x01) return SnowedNull.Instance;
        if (typeByte == 0x02) return SnowedValue.Create(false);
        if (typeByte == 0x03) return SnowedValue.Create(true);

        if (typeByte <= 0x07)
        {
            ulong raw = ReadLE64();
            return typeByte == 0x05 ? SnowedValue.Create(raw) : SnowedValue.Create((long)raw);
        }

        if (typeByte <= 0x0F)
        {
            double f = BitConverter.UInt64BitsToDouble(ReadLE64());
            if (typeByte == 0x09) return SnowedValue.Create(DateTimeOffset.UnixEpoch.AddMilliseconds(f));
            return SnowedValue.Create(f);
        }

        if (typeByte <= 0x3F)
        {
            ulong len = ReadLE64();
            if (len > int.MaxValue) throw Corrupted($"Binary length {len} exceeds platform maximum");
            byte[] payload = ReadBytes((int)len);
            return typeByte switch
            {
                0x11 => ParseString(payload),
                0x12 => ParseDecimal(payload),
                0x13 => SnowedUuid.FromRfc4122Bytes(payload),
                _    => SnowedValue.Create(payload),
            };
        }

        if (typeByte == 0xFF) throw Corrupted("Unexpected Sentinel (0xFF) in value position");
        if (typeByte <= 0x7F) return ReadArray();
        return ReadObject();
    }

    // ── Primitives ────────────────────────────────────────────────────────

    private byte ReadByte()
    {
        if (_pos >= _data.Length) throw Corrupted("Unexpected end of Snowed data");
        return _data[_pos++];
    }

    private ulong ReadLE64()
    {
        if (_pos + 8 > _data.Length) throw Corrupted("Unexpected end of Snowed data");
        ulong val = BinaryPrimitives.ReadUInt64LittleEndian(_data.AsSpan(_pos, 8));
        _pos += 8;
        return val;
    }

    private byte[] ReadBytes(int count)
    {
        if (count < 0 || _pos + count > _data.Length)
            throw Corrupted($"Unexpected end of Snowed data (need {count} bytes)");
        byte[] result = _data[_pos..(_pos + count)];
        _pos += count;
        return result;
    }

    // ── Depth tracking ────────────────────────────────────────────────────

    private void EnterDepth()
    {
        if (++_depth > _maxDepth)
            throw new SnowedException($"Maximum nesting depth {_maxDepth} exceeded");
    }

    private void ExitDepth() => _depth--;

    // ── Error helpers ─────────────────────────────────────────────────────

    private static SnowedException Corrupted(string message, Exception? inner = null)
        => inner is null ? new SnowedException(message) : new SnowedException(message, inner);
}
