using Microsoft.AspNetCore.Mvc;
using Microsoft.Extensions.DependencyInjection;

namespace Snowed.AspNetCore;

/// <summary>
/// Extension methods for configuring Snowed formatters in ASP.NET Core.
/// </summary>
public static class MvcBuilderExtensions
{
    /// <summary>
    /// Adds Snowed input and output formatters, enabling <c>application/snowed</c>
    /// as a supported content type for all controllers.
    /// </summary>
    /// <example>
    /// <code>
    /// builder.Services.AddControllers()
    ///     .AddSnowed();
    /// </code>
    /// </example>
    public static IMvcBuilder AddSnowed(this IMvcBuilder builder,
        SnowedSerializerOptions? options = null)
    {
        builder.Services.Configure<MvcOptions>(mvc =>
        {
            mvc.InputFormatters.Insert(0, new SnowedInputFormatter(options));
            mvc.OutputFormatters.Insert(0, new SnowedOutputFormatter(options));
        });
        return builder;
    }
}
