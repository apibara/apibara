use std::{fmt, fs, path::Path};

use anstyle::{AnsiColor, Style};
use apibara_script::{Script, ScriptOptions};
use clap::builder::Styles;
use error_stack::{Result, ResultExt};
use tokio_util::sync::CancellationToken;

#[derive(Debug)]
pub struct LoadScriptError;
impl error_stack::Context for LoadScriptError {}

impl fmt::Display for LoadScriptError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("failed to load script")
    }
}

pub fn load_script_from_path(
    path: &Path,
    options: ScriptOptions,
) -> Result<Script, LoadScriptError> {
    let path = path.to_string_lossy();
    load_script(&path, options)
}

/// Load a script from a file.
pub fn load_script(path: &str, options: ScriptOptions) -> Result<Script, LoadScriptError> {
    let Ok(_) = fs::metadata(path) else {
        return Err(LoadScriptError)
            .attach_printable_lazy(|| format!("script file not found: {}", path));
    };

    let current_dir = std::env::current_dir()
        .change_context(LoadScriptError)
        .attach_printable("failed to get current directory")?;

    let script = Script::from_file(path, current_dir, options)
        .change_context(LoadScriptError)
        .attach_printable_lazy(|| format!("failed to load script at path: {}", path))?;

    Ok(script)
}

/// Connect the cancellation token to the ctrl-c handler.
pub fn set_ctrlc_handler(ct: CancellationToken) -> Result<(), ctrlc::Error> {
    ctrlc::set_handler({
        move || {
            ct.cancel();
        }
    })
    .attach_printable("failed to register ctrl-c handler")?;

    Ok(())
}

/// A clap style for all Apibara CLI applications.
pub fn apibara_cli_style() -> Styles {
    Styles::styled()
        .header(Style::new().bold().fg_color(Some(AnsiColor::Yellow.into())))
        .error(Style::new().bold().fg_color(Some(AnsiColor::Red.into())))
        .usage(Style::new().bold().fg_color(Some(AnsiColor::Yellow.into())))
        .literal(Style::new().fg_color(Some(AnsiColor::BrightCyan.into())))
        .placeholder(Style::new())
        .valid(Style::new().fg_color(Some(AnsiColor::BrightBlue.into())))
        .invalid(
            Style::new()
                .underline()
                .fg_color(Some(AnsiColor::Red.into())),
        )
}
