using System.Net.Http.Headers;

namespace Snowed.Http;

/// <summary>
/// Extension methods for <see cref="HttpClient"/> that send and receive Snowed-encoded payloads.
/// The content type used is <c>application/snowed</c>.
/// </summary>
public static class HttpClientExtensions
{
    private const string MediaType = "application/snowed";

    // ── GET ───────────────────────────────────────────────────────────────

    /// <summary>
    /// Sends a GET request to <paramref name="requestUri"/> and deserializes the
    /// Snowed response body as <typeparamref name="T"/>.
    /// </summary>
    public static async Task<T?> GetFromSnowedAsync<T>(
        this HttpClient client,
        string requestUri,
        SnowedSerializerOptions? options = null,
        CancellationToken cancellationToken = default)
    {
        using var response = await client.GetAsync(requestUri, cancellationToken).ConfigureAwait(false);
        response.EnsureSuccessStatusCode();
        using var stream = await response.Content.ReadAsStreamAsync(cancellationToken).ConfigureAwait(false);
        return await SnowedSerializer.DeserializeAsync<T>(stream, options, cancellationToken).ConfigureAwait(false);
    }

    // ── POST ──────────────────────────────────────────────────────────────

    /// <summary>
    /// Serializes <paramref name="value"/> to Snowed and POSTs it to <paramref name="requestUri"/>.
    /// </summary>
    public static async Task<HttpResponseMessage> PostAsSnowedAsync<T>(
        this HttpClient client,
        string requestUri,
        T value,
        SnowedSerializerOptions? options = null,
        CancellationToken cancellationToken = default)
    {
        using var content = CreateSnowedContent(value, options);
        return await client.PostAsync(requestUri, content, cancellationToken).ConfigureAwait(false);
    }

    /// <summary>
    /// Serializes <paramref name="value"/> to Snowed, POSTs it, and deserializes the
    /// response body as <typeparamref name="TResponse"/>.
    /// </summary>
    public static async Task<TResponse?> PostAsSnowedAsync<TRequest, TResponse>(
        this HttpClient client,
        string requestUri,
        TRequest value,
        SnowedSerializerOptions? options = null,
        CancellationToken cancellationToken = default)
    {
        using var content = CreateSnowedContent(value, options);
        using var response = await client.PostAsync(requestUri, content, cancellationToken).ConfigureAwait(false);
        response.EnsureSuccessStatusCode();
        using var stream = await response.Content.ReadAsStreamAsync(cancellationToken).ConfigureAwait(false);
        return await SnowedSerializer.DeserializeAsync<TResponse>(stream, options, cancellationToken).ConfigureAwait(false);
    }

    // ── PUT ───────────────────────────────────────────────────────────────

    /// <summary>
    /// Serializes <paramref name="value"/> to Snowed and PUTs it to <paramref name="requestUri"/>.
    /// </summary>
    public static async Task<HttpResponseMessage> PutAsSnowedAsync<T>(
        this HttpClient client,
        string requestUri,
        T value,
        SnowedSerializerOptions? options = null,
        CancellationToken cancellationToken = default)
    {
        using var content = CreateSnowedContent(value, options);
        return await client.PutAsync(requestUri, content, cancellationToken).ConfigureAwait(false);
    }

    // ── PATCH ─────────────────────────────────────────────────────────────

    /// <summary>
    /// Serializes <paramref name="value"/> to Snowed and PATCHes it to <paramref name="requestUri"/>.
    /// </summary>
    public static async Task<HttpResponseMessage> PatchAsSnowedAsync<T>(
        this HttpClient client,
        string requestUri,
        T value,
        SnowedSerializerOptions? options = null,
        CancellationToken cancellationToken = default)
    {
        using var content = CreateSnowedContent(value, options);
        return await client.PatchAsync(requestUri, content, cancellationToken).ConfigureAwait(false);
    }

    // ── Read response ─────────────────────────────────────────────────────

    /// <summary>
    /// Reads the body of <paramref name="response"/> and deserializes it as <typeparamref name="T"/>.
    /// </summary>
    public static async Task<T?> ReadFromSnowedAsync<T>(
        this HttpResponseMessage response,
        SnowedSerializerOptions? options = null,
        CancellationToken cancellationToken = default)
    {
        using var stream = await response.Content.ReadAsStreamAsync(cancellationToken).ConfigureAwait(false);
        return await SnowedSerializer.DeserializeAsync<T>(stream, options, cancellationToken).ConfigureAwait(false);
    }

    // ── Helper ────────────────────────────────────────────────────────────

    private static ByteArrayContent CreateSnowedContent<T>(T value, SnowedSerializerOptions? options)
    {
        byte[] bytes = SnowedSerializer.Serialize(value, options);
        var content = new ByteArrayContent(bytes);
        content.Headers.ContentType = new MediaTypeHeaderValue(MediaType);
        return content;
    }
}
