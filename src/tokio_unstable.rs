use anyhow::Result;
use dropshot::{HttpError, HttpResponseOk, RequestContext, endpoint};
use schemars::JsonSchema;
use serde::Serialize;
use std::{io::Write, time::Duration};
use tokio::{runtime::Handle, sync::oneshot, time::timeout};

use crate::Context;

#[derive(Clone, Debug, PartialEq, JsonSchema, Serialize)]
struct StacktraceTokioResponse {
    stacktrace: String,
}

/// Request a tokio stacktrace.
///
/// This method *may* result in the tokio runtime totally crashing on you.
/// If this happens, the HTTP response won't be returned, but we'll do our
/// best to catch that state and crash the process right after printing
/// the backtrace to stderr.
#[endpoint {
    method = GET,
    path = "/stacktrace/tokio",
}]
pub(super) async fn stacktrace_tokio(
    _rqctx: RequestContext<Context>,
) -> Result<HttpResponseOk<StacktraceTokioResponse>, HttpError> {
    let (stack_tx, stack_rx) = oneshot::channel();
    let (ack_tx, mut ack_rx) = oneshot::channel();

    let handle = Handle::current();
    std::thread::spawn(move || {
        // We need to escape the async runtime as much as we can in case
        // this goes very very south.

        handle.block_on(async move {
            let handle = Handle::current();
            let mut bytes = Vec::new();
            match timeout(Duration::from_secs(10), handle.dump()).await {
                Ok(dump) => {
                    for (i, task) in dump.tasks().iter().enumerate() {
                        let trace = task.trace();
                        let _ = write!(&mut bytes, "\nTokio Task ID {i}:\n");
                        let _ = write!(&mut bytes, "{trace}\n\n\n");
                    }
                    // We don't need the result because of our timeout handling
                    // in a sec.
                    let _ = stack_tx.send(bytes.clone());
                }
                Err(_) => {
                    // send nil
                    let _ = writeln!(
                        &mut bytes,
                        "Internal error: tokio was unable to complete a backtrace"
                    );
                    let _ = stack_tx.send(bytes.clone());
                }
            };

            // Here we don't want to give up our async runtime because
            // the runtime may be horked. We're going to wedge open our
            // thread and then try to recv. If we hit an error we know
            // the other end hasn't run yet. If we're OK the user has the
            // message and we can return.

            std::thread::sleep(Duration::from_secs(1));
            if ack_rx.try_recv().is_ok() {
                // User got the bytes via the HTTP response,
                // so we're good to unwind here.
                return;
            }

            // If we're here, I'm sorry to report that tokio's runtime
            // is prob totally fucked. We need to get this message back
            // to the user, and it's not like this hunk of junk will be
            // serving any requests anymore, so let's bail.

            eprintln!("{}", String::from_utf8(bytes).unwrap());

            // construct a conspicuous return number so that we
            // know that this wasn't from nature or something
            // serious like the operating system.
            std::process::exit(1337);
        });
    });

    match stack_rx.await {
        Ok(bytes) => {
            let _ = ack_tx.send(true);
            Ok(HttpResponseOk(StacktraceTokioResponse {
                stacktrace: String::from_utf8(bytes).unwrap().to_string(),
            }))
        }
        Err(e) => Err(HttpError::for_internal_error(format!(
            "internal error: {}",
            e
        ))),
    }
}
