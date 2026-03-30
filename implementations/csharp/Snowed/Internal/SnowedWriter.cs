using System.Buffers.Binary;
using System.Text;

namespace Snowed.Internal;

/// <summary>Writes Snowed values to a Stream.</summary>
internal sealed class SnowedWriter
{
    private readonly Stream _stream;

    internal SnowedWriter(Stream stream) { _stream = stream; }

    // ── Top-level dispatch ────────────────────────────────────────────────

    internal void Write(SnowedValue value)
    {
        switch (value)
        {
            case SnowedNull:          WriteNull();                          break;
            case SnowedBoolean b:     WriteBoolean(b.Value);                break;
            case SnowedInt64 i:       WriteInt64(i.Value);                  break;
            case SnowedUInt64 u:      WriteUInt64(u.Value);                 break;
            case SnowedFloat64 f:     WriteFloat64(f.Value);                break;
            case SnowedDateTime dt:   WriteDateTime(dt.Value);              break;
            case SnowedBinary bin:    WriteBinary(bin.Value.Span);          break;
            case SnowedString s:      WriteString(s.Value);                 break;
            case SnowedDecimal d:     WriteDecimal(d.Value);                break;
            case SnowedUuid u:        WriteUuid(u);                         break;
            case SnowedArray arr:     WriteArray(arr);                      break;
            case SnowedObject obj:    WriteObject(obj);                     break;
            default: throw new SnowedException($"Unhandled SnowedValue type: {value.GetType().Name}");
        }
    }

    // ── Scalar writers ────────────────────────────────────────────────────

    internal void WriteNull()             => _stream.WriteByte(0x00);
    internal void WriteBoolean(bool v)    => _stream.WriteByte(v ? (byte)0x03 : (byte)0x02);

    internal void WriteInt64(long v)
    {
        _stream.WriteByte(0x04);
        WriteLE64((ulong)v);
    }

    internal void WriteUInt64(ulong v)
    {
        _stream.WriteByte(0x05);
        WriteLE64(v);
    }

    internal void WriteFloat64(double v)
    {
        _stream.WriteByte(0x08);
        WriteLE64(BitConverter.DoubleToUInt64Bits(v));
    }

    internal void WriteDateTime(DateTimeOffset v)
    {
        double ms = (v.ToUniversalTime() - DateTimeOffset.UnixEpoch).TotalMilliseconds;
        _stream.WriteByte(0x09);
        WriteLE64(BitConverter.DoubleToUInt64Bits(ms));
    }

    internal void WriteBinary(ReadOnlySpan<byte> payload)
    {
        _stream.WriteByte(0x10);
        WriteLE64((ulong)payload.Length);
        _stream.Write(payload);
    }

    internal void WriteString(string v)
    {
        byte[] utf8 = Encoding.UTF8.GetBytes(v);
        _stream.WriteByte(0x11);
        WriteLE64((ulong)utf8.Length);
        _stream.Write(utf8);
    }

    internal void WriteDecimal(decimal v)
    {
        byte[] utf8 = Encoding.UTF8.GetBytes(SnowedDecimal.CanonicalDecimalString(v));
        _stream.WriteByte(0x12);
        WriteLE64((ulong)utf8.Length);
        _stream.Write(utf8);
    }

    internal void WriteUuid(SnowedUuid u)
    {
        byte[] rfc4122 = u.ToRfc4122Bytes();
        _stream.WriteByte(0x13);
        WriteLE64(16);
        _stream.Write(rfc4122);
    }

    internal void WriteArray(SnowedArray arr)
    {
        _stream.WriteByte(0x40);
        foreach (var item in arr.Items)
            Write(item);
        _stream.WriteByte(0xFF);                            // Sentinel
    }

    internal void WriteObject(SnowedObject obj)
    {
        _stream.WriteByte(0x80);
        // Sort keys for deterministic output
        foreach (var kv in obj.Pairs.OrderBy(p => p.Key, StringComparer.Ordinal))
        {
            WriteObjectKey(kv.Key);
            Write(kv.Value);
        }
        _stream.WriteByte(0x00);                            // zero-length key terminator
    }

    // ── Helpers ───────────────────────────────────────────────────────────

    private void WriteObjectKey(string key)
    {
        byte[] utf8 = Encoding.UTF8.GetBytes(key);
        if (utf8.Length > 255)
            throw new SnowedException($"Object key '{key}' exceeds 255 UTF-8 bytes ({utf8.Length})");
        _stream.WriteByte((byte)utf8.Length);
        _stream.Write(utf8);
    }

    private void WriteLE64(ulong value)
    {
        Span<byte> buf = stackalloc byte[8];
        BinaryPrimitives.WriteUInt64LittleEndian(buf, value);
        _stream.Write(buf);
    }
}
