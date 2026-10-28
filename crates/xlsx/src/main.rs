use clap::Parser;
use rhodopsin_xlsx::{
    diagnostic::{Category, Failure},
    package::Limits,
};
use std::{io::Write, path::PathBuf, process::ExitCode};

#[derive(Parser)]
#[command(
    version,
    about = "Convert printed XLSX pages to PDF (under development)"
)]
struct Args {
    /// Input .xlsx
    input: PathBuf,
    /// Output PDF (existing files are rejected)
    #[arg(long)]
    out: PathBuf,
    /// Font file allowed for loading (repeatable)
    #[arg(long)]
    font_file: Vec<PathBuf>,
    /// Font directory allowed for searching (repeatable)
    #[arg(long)]
    font_dir: Vec<PathBuf>,
    /// Substitute family used only when a requested font is missing (none by default)
    #[arg(long)]
    fallback_font: Option<String>,
    /// Also search the OS font directories
    #[arg(long)]
    system_fonts: bool,
}

fn fail(error: Failure) -> ExitCode {
    let code = error.category.exit_code();
    let stderr = std::io::stderr();
    let mut writer = stderr.lock();
    let _ = serde_json::to_writer(&mut writer, &error);
    let _ = writeln!(writer);
    ExitCode::from(code)
}

// Called only after the PDF is committed. A stdout failure must not exit successfully.
fn write_success(
    writer: &mut impl Write,
    report: &rhodopsin_xlsx::report::Report,
) -> std::io::Result<()> {
    serde_json::to_writer(&mut *writer, report)?;
    writeln!(writer)?;
    writer.flush()
}

fn success(report: &rhodopsin_xlsx::report::Report) -> ExitCode {
    let stdout = std::io::stdout();
    match write_success(&mut stdout.lock(), report) {
        Ok(()) => ExitCode::SUCCESS,
        Err(_) => fail(Failure::new(
            Category::InternalError,
            "success_report_write_failed",
        )),
    }
}

fn main() -> ExitCode {
    // Panics from libraries must not leak input-derived strings into diagnostics either.
    std::panic::set_hook(Box::new(|_| {}));
    let result = std::panic::catch_unwind(|| {
        let args = match Args::try_parse() {
            Ok(args) => args,
            Err(e)
                if matches!(
                    e.kind(),
                    clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion
                ) =>
            {
                let _ = e.print();
                return ExitCode::SUCCESS;
            }
            Err(_) => return fail(Failure::input("invalid_arguments")),
        };
        let options = rhodopsin_xlsx::ConversionOptions {
            font_sources: rhodopsin_xlsx::font_sources::Sources {
                files: args.font_file,
                directories: args.font_dir,
            },
            fallback_family: args.fallback_font,
            system_fonts: args.system_fonts,
            ..Default::default()
        };
        match rhodopsin_xlsx::convert_with_options(
            &args.input,
            &args.out,
            &Limits::default(),
            &options,
        ) {
            Ok(report) => success(&report),
            Err(error) => fail(error),
        }
    });
    result.unwrap_or_else(|_| fail(Failure::new(Category::InternalError, "internal_panic")))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn report() -> rhodopsin_xlsx::report::Report {
        rhodopsin_xlsx::report::Report {
            schema_version: 1,
            page_count: 0,
            pages: vec![],
            fonts: vec![],
            warnings: vec![],
            layout_font_substituted: false,
        }
    }
    #[test]
    fn 成功情報を改行付きjsonとして出力する() {
        let mut bytes = Vec::new();
        write_success(&mut bytes, &report()).unwrap();
        assert!(bytes.ends_with(b"\n"));
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(json["schema_version"], 1);
    }
    #[test]
    fn 標準出力の失敗を返す() {
        struct Broken;
        impl Write for Broken {
            fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
                Err(std::io::ErrorKind::BrokenPipe.into())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        assert!(write_success(&mut Broken, &report()).is_err());
    }
}
