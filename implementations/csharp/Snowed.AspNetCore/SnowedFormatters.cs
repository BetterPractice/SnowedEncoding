using Microsoft.AspNetCore.Mvc.Formatters;

namespace Snowed.AspNetCore;

/// <summary>
/// ASP.NET Core input formatter that reads <c>application/snowed</c> request bodies.
/// </summary>
public sealed class SnowedInputFormatter : InputFormatter
{
    private const string MediaType = "application/snowed";

    private readonly SnowedSerializerOptions _options;

    public SnowedInputFormatter(SnowedSerializerOptions? options = null)
    {
        _options = options ?? SnowedSerializerOptions.Default;
        SupportedMediaTypes.Add(MediaType);
    }

    protected override bool CanReadType(Type type) => true;

    public override async Task<InputFormatterResult> ReadRequestBodyAsync(InputFormatterContext context)
    {
        var request = context.HttpContext.Request;
        try
        {
            object? model = await DeserializeAsync(
                request.Body,
                context.ModelType,
                _options,
                context.HttpContext.RequestAborted
            ).ConfigureAwait(false);
            return InputFormatterResult.Success(model);
        }
        catch (SnowedException ex)
        {
            context.ModelState.AddModelError(string.Empty, ex.Message);
            return InputFormatterResult.Failure();
        }
    }

    // Helper that the serializer can use with a non-generic Type
    private static Task<object?> DeserializeAsync(Stream stream, Type type,
        SnowedSerializerOptions options, CancellationToken ct)
    {
        // Delegate to the generic method via reflection — acceptable here since this
        // is the ASP.NET Core model-binding path, not a hot serialization loop.
        var method = typeof(SnowedSerializer)
            .GetMethod(nameof(SnowedSerializer.DeserializeAsync))!
            .MakeGenericMethod(type);
        var task = (Task)method.Invoke(null, new object[] { stream, options, ct })!;
        return ConvertTask(task, type);
    }

    private static async Task<object?> ConvertTask(Task task, Type type)
    {
        await task.ConfigureAwait(false);
        return ((dynamic)task).Result;
    }
}

/// <summary>
/// ASP.NET Core output formatter that writes <c>application/snowed</c> response bodies.
/// </summary>
public sealed class SnowedOutputFormatter : OutputFormatter
{
    private const string MediaType = "application/snowed";

    private readonly SnowedSerializerOptions _options;

    public SnowedOutputFormatter(SnowedSerializerOptions? options = null)
    {
        _options = options ?? SnowedSerializerOptions.Default;
        SupportedMediaTypes.Add(MediaType);
    }

    protected override bool CanWriteType(Type? type) => true;

    public override async Task WriteResponseBodyAsync(OutputFormatterWriteContext context)
    {
        var response = context.HttpContext.Response;
        response.ContentType = MediaType;
        await SnowedSerializer.SerializeAsync(
            response.Body,
            context.Object,
            _options,
            context.HttpContext.RequestAborted
        ).ConfigureAwait(false);
    }

    private static Task SerializeAsync(Stream stream, object? value, Type type,
        SnowedSerializerOptions options, CancellationToken ct)
    {
        var method = typeof(SnowedSerializer)
            .GetMethod(nameof(SnowedSerializer.SerializeAsync))!
            .MakeGenericMethod(type);
        return (Task)method.Invoke(null, new object?[] { stream, value, options, ct })!;
    }
}
