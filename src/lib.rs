#![deny(missing_docs)]

//! This crate implements support for the `delouse` daemon, a set of HTTP
//! endpoints that enable specific debugging capabilities.

use anyhow::Result;
use dropshot::{
    endpoint, ApiDescription, ConfigLogging, ConfigLoggingLevel, HttpError, HttpResponseOk,
    HttpServerStarter, RequestContext,
};
use schemars::JsonSchema;
use serde::Serialize;

#[cfg(all(tokio_unstable, tokio_taskdump))]
mod tokio_unstable;

#[cfg(target_os = "linux")]
mod os_linux;
#[cfg(target_os = "linux")]
use os_linux as os;

#[cfg(target_os = "macos")]
mod os_macos;

struct Context {
    schema: serde_json::Value,
}

/// Return the OpenAPI schema for the daemon as-configured. Some endpoints
/// may differ depending on the host that is being targeted.
#[endpoint {
    method = GET,
    path = "/",
}]
pub async fn get_schema(
    rqctx: RequestContext<Context>,
) -> Result<HttpResponseOk<serde_json::Value>, HttpError> {
    let context = rqctx.context();

    Ok(HttpResponseOk(context.schema.clone()))
}

/// Return type for the stacktrace request.
#[derive(Clone, Debug, PartialEq, JsonSchema, Serialize)]
struct StacktraceRustResponse {
    /// Human-readable Rust backtrace.
    stacktrace: String,
}

/// Request a rust stacktrace.
///
/// This endpoint returns the human-readable (well, debatable on some days)
/// stack trace for all currently executing Rust code scheduled on **OS**
/// threads. If you need information on executing functions outside of
/// currently executing threads, the `tokio` specific endpoint may be
/// interesting if supported by your OS.
#[endpoint {
    method = GET,
    path = "/stacktrace/rust",
}]
async fn stacktrace_rust(
    _rqctx: RequestContext<Context>,
) -> Result<HttpResponseOk<StacktraceRustResponse>, HttpError> {
    Ok(HttpResponseOk(StacktraceRustResponse {
        stacktrace: format!("{}", std::backtrace::Backtrace::force_capture()),
    }))
}

/// Initialize the `delouse` internals, and start up a webserver
/// used for pulling debug information from the internals of the
/// resident program.
///
/// Note: This **MUST** be run from inside an async runtime, even though
/// this function is sync, or the call may (should) panic.
///
pub fn init() -> Result<()> {
    let handle = tokio::runtime::Handle::current();

    let mut api = ApiDescription::new();
    api.register(get_schema).unwrap();
    api.register(stacktrace_rust).unwrap();

    #[cfg(all(tokio_unstable, tokio_taskdump))]
    {
        api.register(crate::tokio_unstable::stacktrace_tokio)
            .unwrap();
    }

    #[cfg(target_os = "linux")]
    {
        // linux specific endpoints here
        api.register(crate::os::coredump).unwrap();
        api.register(crate::os::elf_info).unwrap();
    }

    #[cfg(target_os = "macos")]
    {
        // macos specific endpoints here
    }

    let definition = api.openapi("debugd http interface", clap::crate_version!());
    let api_context = Context {
        schema: definition.json().unwrap(),
    };

    let config_logging = ConfigLogging::StderrTerminal {
        level: ConfigLoggingLevel::Info,
    };
    let log = config_logging.to_logger("debugd").unwrap();

    let bind_address = "127.0.0.1:7132";

    let config_dropshot = ConfigDropshot {
        bind_address: bind_address.parse().unwrap(),
        ..Default::default()
    };

    let server = HttpServerStarter::new(&config_dropshot, api, api_context, &log)
        .unwrap()
        .start();

    handle.spawn(server);
    Ok(())
}
