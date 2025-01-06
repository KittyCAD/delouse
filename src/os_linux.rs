use anyhow::Result;
use dropshot::{endpoint, HttpError, HttpResponseOk, RequestContext};
use schemars::JsonSchema;
use serde::Serialize;

use crate::Context;

/// Response from a coredump (you'll never see one of these), since if
/// a coredump is successful the process will be killed, and if not
/// there should be an HTTP Error returned.
#[derive(Clone, Debug, PartialEq, JsonSchema, Serialize)]
struct CoredumpResponse {}

/// Request a coredump
///
/// This endpoint is specific to linux runtimes until additional OS support
/// is added.
///
/// This will attempt to raise the coredump ulimit to the system maxiumum,
/// and induce a crash by triggering an abort.
#[endpoint {
    method = GET,
    path = "/coredump"
}]
pub(crate) async fn coredump(
    _rqctx: RequestContext<Context>,
) -> Result<HttpResponseOk<CoredumpResponse>, HttpError> {
    // TODO: README!
    //
    // OK nerds, this is actually not Linux specific, this should work
    // on any UNIX-like, but I want someone to write text similar to the
    // one I wrote for Linux for OSX, OpenBSD, etc.
    //
    // If you're doing this work, please move this into an `os.rs` or
    // `os_unix.rs`, and fiddle with the paths so that the ELF code stays
    // linux specific but this winds up in some unix-family file. I tried
    // to already move the Linux specific stuff in anticipation of this.

    // Before we begin, let's try and set some coredump ulimits to their max values
    // to give us the best shot.
    let mut rlimit = libc::rlimit {
        rlim_cur: 0,
        rlim_max: 0,
    };
    unsafe {
        // this can fail, but if it does we're left with all zeros which,
        // sorry bud.
        libc::getrlimit(libc::RLIMIT_CORE, &mut rlimit);
    };

    eprintln!(
        "attempting to raise coredump ulimit from {} to {}",
        rlimit.rlim_cur, rlimit.rlim_max
    );

    // set the current limit to our environmental max.
    rlimit.rlim_cur = rlimit.rlim_max;

    let bin_path = std::env::current_exe().unwrap_or("<PATH_TO_THIS_PROGRAM>".into());

    #[cfg(target_os = "linux")]
    {
        eprintln!(
            "

          about to trigger a coredump for inspection 🕵️


The next steps are going to be loading this core into a debugger to
try and figure out what's been going on with the process. If you're
running locally, you can load the core by using gdb.

  - If the coredump isn't where you expect, check where your system
    generates coredumps by running:

    $ cat /proc/sys/kernel/core_pattern

    The meaning of the %* patterns is defined in `man 5 core`.

  - If it's still not where you expect, double check the binary is
    capable of writing to the location in `core_pattern`.

  - Still can't find the core? Check your coredump ulimit, and make
    sure that this is large enough (in bytes) for the coredump you're
    generating.

    $ ulimit -c -H

  - Load the binary in `gdb` using a command that's similar to:

    $ rust-gdb {:?} core

    Adjusting `core` to be where your corefile is located based on your
    system.
",
            bin_path,
        );
    }

    unsafe {
        libc::setrlimit(libc::RLIMIT_CORE, &rlimit);
        libc::abort();
    };

    // Ok(HttpResponseOk(CoredumpResponse {}))
}

/// Return extracted information about our "own" ELF file.
///
/// This is a WIP, more information is welcome.
#[derive(Clone, Debug, PartialEq, JsonSchema, Serialize)]
struct ElfInfoResponse {
    /// Return the `.comments` section of the ELF as a string vector.
    /// This is often used by the compiler to mark what was used to
    /// produce this binary.
    comments: Vec<String>,
}

/// Request that the `delouse` server load up our "own" binary and attempt
/// to extract information which can be pertinent when debugging system
/// issues.
///
/// This is currently Linux specific. Other binary formats (like Mach-O on
/// OSX) are not supported yet.
#[endpoint {
    method = GET,
    path = "/elf/info"
}]
pub(crate) async fn elf_info(
    _rqctx: RequestContext<Context>,
) -> Result<HttpResponseOk<ElfInfoResponse>, HttpError> {
    let bin_path = std::env::current_exe().unwrap();
    let bin_bytes = std::fs::read(bin_path).unwrap();
    let bin = elf::ElfBytes::<elf::endian::AnyEndian>::minimal_parse(&bin_bytes).unwrap();

    let comments = bin.section_header_by_name(".comment").unwrap().unwrap();
    let (comments_bytes, comments_compression) = bin.section_data(&comments).unwrap();
    assert_eq!(None, comments_compression);

    let comments_vec = comments_bytes
        .split(|ch| *ch == 0x00)
        .into_iter()
        .map(|bytes| String::from_utf8(bytes.into()).unwrap())
        .filter(|str| str.len() > 0)
        .collect();

    Ok(HttpResponseOk(ElfInfoResponse {
        comments: comments_vec,
    }))
}
