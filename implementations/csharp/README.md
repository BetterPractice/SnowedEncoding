# Snowed for .NET

C# implementation of the [Snowed binary encoding](https://github.com/BetterPractice/SnowedEncoding): a self-describing, streaming-friendly wire format with native `Binary`, `UUID`, `DateTime`, and `Decimal` types. The wire format is defined by the [specification](https://github.com/BetterPractice/SnowedEncoding/blob/main/spec/snowed-spec.md); this package is one of several conforming implementations.

| Package | Contents |
|---------|----------|
| `BetterPractice.Snowed` | Serializer, document object model, `HttpClient` extensions |
| `BetterPractice.Snowed.AspNetCore` | ASP.NET Core MVC input and output formatters |

```
dotnet add package BetterPractice.Snowed
dotnet add package BetterPractice.Snowed.AspNetCore   # servers only
```

Targets `net8.0` and `net10.0`. No dependencies beyond the framework.

## Serializing objects

```csharp
using Snowed;

public sealed class Patient
{
    [SnowedPropertyName("patient_id")]
    public required Guid PatientId { get; init; }

    [SnowedPropertyName("created_at")]
    public required DateTimeOffset CreatedAt { get; init; }

    [SnowedPropertyName("data")]
    public required byte[] Data { get; init; }      // encoded as native Binary, no base64

    [SnowedIgnore]
    public string? Transient { get; set; }
}

byte[] bytes = SnowedSerializer.Serialize(patient);
Patient? back = SnowedSerializer.Deserialize<Patient>(bytes);
```

Stream overloads exist for both directions, including `SerializeAsync` and `DeserializeAsync`, so large payloads never need to be buffered in full.

`SnowedSerializerOptions` controls naming policy, converters, and how unknown fields and duplicate keys are handled. Custom types implement `SnowedConverter<T>`.

## Document model

When the shape is not known ahead of time, work with the value tree directly:

```csharp
SnowedValue value = SnowedSerializer.DeserializeValue(bytes);
if (value is SnowedObject obj && obj["patient_id"] is SnowedUuid id)
{
    // ...
}

byte[] encoded = SnowedSerializer.Encode(obj);
```

## HttpClient

The `Snowed.Http` namespace adds Snowed equivalents of the JSON `HttpClient` helpers. Requests are sent with content type `application/snowed`.

```csharp
using Snowed.Http;

var created = await client.PostAsSnowedAsync<CreateRequest, CreateResponse>("/patients", request);
var patient = await client.GetFromSnowedAsync<Patient>($"/patients/{id}");
```

`PutAsSnowedAsync`, `PatchAsSnowedAsync`, and `ReadFromSnowedAsync` on `HttpContent` are also provided.

## ASP.NET Core

```csharp
builder.Services.AddControllers().AddSnowed();
```

Registers input and output formatters for `application/snowed`. Controllers then bind `[FromBody]` parameters and return results in Snowed with no further changes. Pass a `SnowedSerializerOptions` to `AddSnowed` to customize serialization server-wide.

## Versioning

Versions below 1.0 signal that the library API may still change. The wire format is governed by the specification, and every implementation in the repository is validated against the shared conformance vectors.

## License

MIT. See [LICENSE](https://github.com/BetterPractice/SnowedEncoding/blob/main/LICENSE).
