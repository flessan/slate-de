//! `vision` — the modular vision framework, from the prompt.

use std::path::PathBuf;

use slate_core::error::{Error, Result};
use slate_core::proc;
use slate_core::style::{Line, Span};
use slate_vision::{provider, Provider, PROVIDERS};

use crate::{Command, Ctx, Output, Registry};

pub fn register(r: &mut Registry) {
    r.register(VisionCmd);
}

struct VisionCmd;

impl Command for VisionCmd {
    fn name(&self) -> &'static str {
        "vision"
    }
    fn about(&self) -> &'static str {
        "vision framework: lens, OCR, QR, translation, screenshots"
    }
    fn usage(&self) -> &'static str {
        "vision [providers|use <p>|<file>|<url>|lens <url>|qr <f>|ocr <f>|translate <f>|screenshot|clipboard]"
    }

    fn run(&self, args: &[String], ctx: &mut Ctx) -> Result<Output> {
        let theme = ctx.theme();
        match args.first().map(String::as_str) {
            None | Some("providers") => {
                let mut out = Output::new();
                out.line(Line::styled(
                    format!("vision providers (default: {})", ctx.vision.default_name()),
                    theme.accent(),
                ));
                for p in PROVIDERS {
                    let default = p.name == ctx.vision.default_name();
                    let (glyph, style) =
                        if default { ("●", theme.accent()) } else { (" ", theme.text()) };
                    let tool_note = match p.required_tool() {
                        Some(tool) if !proc::exists(tool) => {
                            format!(" — needs `{tool}` (missing)")
                        }
                        _ => String::new(),
                    };
                    out.line(Line::new(vec![
                        Span::styled(format!("{glyph} "), style),
                        Span::styled(format!("{:<10}", p.name), style),
                        Span::styled(format!("{p}{tool_note}", p = p.summary), theme.dim()),
                    ]));
                }
                Ok(out)
            }
            Some("use") => {
                let name = args.get(1).ok_or_else(|| Error::invalid("usage: vision use <provider>"))?;
                if !ctx.vision.set_default(name) {
                    return Err(Error::not_found(format!("vision provider '{name}'")));
                }
                let _ = ctx.cfg.set_and_save("vision.default_provider", name);
                Ok(Output::text(format!("vision provider: {name}")))
            }
            Some("lens") => lens_like(args.get(1), provider("lens").unwrap_or(&PROVIDERS[0]), ctx),
            Some("bing") => lens_like(args.get(1), provider("bing").unwrap_or(&PROVIDERS[0]), ctx),
            Some("yandex") => lens_like(args.get(1), provider("yandex").unwrap_or(&PROVIDERS[0]), ctx),
            Some("qr") | Some("scan") => run_tool(provider("qr").unwrap_or(&PROVIDERS[4]), args.get(1), ctx),
            Some("ocr") => run_tool(provider("ocr").unwrap_or(&PROVIDERS[3]), args.get(1), ctx),
            Some("translate") => translate(args.get(1), ctx),
            Some("screenshot") => screenshot(ctx),
            Some("clipboard") => clipboard_image(ctx),
            Some("file") => default_route(args.get(1), ctx),
            Some(target) => {
                // `vision <something>`: treat as file/URL via default provider.
                default_route(Some(target), ctx)
            }
        }
    }
}

fn is_url(s: &str) -> bool {
    s.starts_with("http://") || s.starts_with("https://")
}

fn resolve_file(ctx: &Ctx, arg: Option<&String>) -> Result<PathBuf> {
    let Some(a) = arg else {
        return Err(Error::invalid("missing file or url argument"));
    };
    let p = PathBuf::from(crate::ctx::expand_tilde(a));
    let p = if p.is_absolute() { p } else { ctx.cwd.join(p) };
    if !p.exists() {
        return Err(Error::not_found(format!("file '{}'", p.display())));
    }
    Ok(p)
}

/// Browser-style providers need a public URL. For local files we explain and
/// open the provider's home page (upload happens in the browser).
fn lens_like(arg: Option<&String>, p: &Provider, ctx: &mut Ctx) -> Result<Output> {
    let Some(a) = arg else {
        return Err(Error::invalid(format!("usage: vision {} <image-url|file>", p.name)));
    };
    let theme = ctx.theme();
    if is_url(a) {
        let url = p.url_for(a).unwrap_or_default();
        proc::open_target(&url)?;
        return Ok(Output::text(format!("opened {} in your browser ({})", p.label, p.name)));
    }
    let file = resolve_file(ctx, Some(a))?;
    let mut out = Output::new();
    out.line(Line::styled(
        format!("{} works on public image URLs; local files upload via the browser:", p.label),
        theme.text(),
    ));
    out.line(Line::styled(format!("  file: {}", file.display()), theme.dim()));
    let home = match p.name {
        "bing" => "https://www.bing.com/visualsearch",
        "yandex" => "https://yandex.com/images/",
        _ => "https://lens.google.com/",
    };
    out.line(Line::styled(format!("  opening {home} — drag the file in",), theme.dim()));
    proc::open_target(home)?;
    Ok(out)
}

/// Default-provider routing for `vision <target>`.
fn default_route(arg: Option<&String>, ctx: &mut Ctx) -> Result<Output> {
    let Some(a) = arg else {
        return Err(Error::invalid("usage: vision <file|url> (or `vision providers`)"));
    };
    let p = ctx.vision.default_provider();
    match p.kind {
        slate_vision::ProviderKind::ByUrl { .. } => lens_like(Some(a), p, ctx),
        slate_vision::ProviderKind::Tool { .. } => run_tool(p, Some(a), ctx),
        slate_vision::ProviderKind::OcrTranslate => translate(Some(a), ctx),
    }
}

fn run_tool(p: &Provider, arg: Option<&String>, ctx: &mut Ctx) -> Result<Output> {
    let theme = ctx.theme();
    if is_url(arg.map(String::as_str).unwrap_or("")) {
        return Err(Error::invalid(
            "this provider needs a local file — try `vision screenshot` or download first",
        ));
    }
    let Some((tool, argv)) = p.tool_argv("") else {
        return Err(Error::other("provider is not a tool"));
    };
    if !proc::exists(&tool) {
        return Err(Error::unavailable(format!(
            "`{tool}` is not installed. Install it, or switch provider: `vision use lens`"
        )));
    }
    let file = resolve_file(ctx, arg)?;
    let argv: Vec<String> = p.tool_argv(&file.to_string_lossy()).map(|(_, a)| a).unwrap_or(argv);
    let args: Vec<&str> = argv.iter().map(String::as_str).collect();
    let out = proc::run(&tool, &args)?;
    let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
    let mut output = Output::new();
    if stdout.is_empty() {
        output.line(Line::styled(format!("({}: no text detected)", p.name), theme.dim()));
    } else {
        for line in stdout.lines().take(100) {
            output.line(Line::styled(line.to_string(), theme.text()));
        }
    }
    if !out.status.success() && !stderr.is_empty() {
        output.line(Line::styled(stderr, theme.warn()));
    }
    Ok(output)
}

fn translate(arg: Option<&String>, ctx: &mut Ctx) -> Result<Output> {
    let theme = ctx.theme();
    let file = resolve_file(ctx, arg)?;
    let ocr = provider("ocr").ok_or_else(|| Error::other("ocr provider missing"))?;
    let Some((tool, argv)) = ocr.tool_argv(&file.to_string_lossy()) else {
        return Err(Error::other("ocr provider misconfigured"));
    };
    if !proc::exists(&tool) {
        return Err(Error::unavailable("OCR needs `tesseract` installed"));
    }
    let args: Vec<&str> = argv.iter().map(String::as_str).collect();
    let out = proc::run(&tool, &args)?;
    let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if text.is_empty() {
        return Ok(Output::text("(no text detected in image)"));
    }
    let mut output = Output::new();
    output.line(Line::styled("extracted text:", theme.dim()));
    for line in text.lines().take(30) {
        output.line(Line::styled(format!("  {line}"), theme.text()));
    }
    if proc::exists("trans") {
        let t = proc::run_trimmed("trans", &["-b", ":en", text.as_str()]);
        if let Some(translation) = t {
            output.line(Line::styled("translation:", theme.accent()));
            for line in translation.lines().take(30) {
                output.line(Line::styled(format!("  {line}"), theme.text()));
            }
            return Ok(output);
        }
    }
    output.line(Line::styled(
        "(install translate-shell `trans` for automatic translation)",
        theme.dim(),
    ));
    Ok(output)
}

fn screenshot(ctx: &mut Ctx) -> Result<Output> {
    let path = std::env::temp_dir().join("slate-screenshot.png");
    let captured = if proc::exists("grim") {
        proc::run("grim", &[&path.to_string_lossy()]).map(|o| o.status.success()).unwrap_or(false)
    } else if proc::exists("import") {
        proc::run("import", &["-window", "root", &path.to_string_lossy()])
            .map(|o| o.status.success())
            .unwrap_or(false)
    } else {
        false
    };
    if !captured {
        return Err(Error::unavailable(
            "screenshot needs `grim` (Wayland) or `import` (ImageMagick/X11)",
        ));
    }
    let path_str = path.to_string_lossy().into_owned();
    default_route(Some(&path_str), ctx)
}

fn clipboard_image(ctx: &mut Ctx) -> Result<Output> {
    let path = std::env::temp_dir().join("slate-clipboard.png");
    let path_str = path.to_string_lossy().into_owned();
    let ok = if proc::exists("wl-paste") {
        let out = proc::run("wl-paste", &["-t", "image/png"])?;
        if out.status.success() && !out.stdout.is_empty() {
            std::fs::write(&path, &out.stdout).is_ok()
        } else {
            false
        }
    } else if proc::exists("xclip") {
        proc::run("xclip", &["-selection", "clipboard", "-t", "image/png", "-o", path_str.as_str()])
            .map(|o| o.status.success())
            .unwrap_or(false)
    } else {
        false
    };
    if !ok {
        return Err(Error::unavailable(
            "no clipboard image (needs `wl-paste` on Wayland or `xclip` on X11, and an image in the clipboard)",
        ));
    }
    default_route(Some(&path_str), ctx)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn providers_and_default_switch() {
        let r = Registry::builtins();
        let mut ctx = Ctx::ephemeral();
        ctx.cfg.path =
            std::env::temp_dir().join(format!("slate-visioncmd-{}.toml", std::process::id()));

        let out = r.exec("vision providers", &mut ctx).unwrap();
        let text = out.to_plain_string();
        assert!(text.contains("lens"));
        assert!(text.contains("tesseract"));

        r.exec("vision use ocr", &mut ctx).unwrap();
        assert_eq!(ctx.vision.default_name(), "ocr");
        assert!(r.exec("vision use nope", &mut ctx).is_err());

        let _ = std::fs::remove_file(&ctx.cfg.path);
    }

    #[test]
    fn missing_file_errors_cleanly() {
        let r = Registry::builtins();
        let mut ctx = Ctx::ephemeral();
        assert!(r.exec("vision qr /no/such/file.png", &mut ctx).is_err());
        assert!(r.exec("vision file /no/such/file.png", &mut ctx).is_err());
    }
}
