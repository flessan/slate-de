//! `slate-vision` — the modular vision framework.
//!
//! Vision providers are *pluggable backends* behind a small, stable spec:
//!
//! * **web providers** (Google Lens, Bing Visual Search, Yandex) — build the
//!   provider URL from a public image URL; Slate opens the user's default
//!   browser
//! * **tool providers** (tesseract OCR, zbarimg QR/barcode, translate-shell)
//!   — run local tools when present
//!
//! New providers = new entries in [`PROVIDERS`] (or, later, WASM plugins).

#![forbid(unsafe_code)]

use slate_core::url::percent_encode;

/// How a provider resolves an image.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProviderKind {
    /// Open the browser at `build(image_url)`.
    ByUrl { build: fn(&str) -> String },
    /// Run a local tool: `tool` with `{file}` placeholders expanded in `args`.
    Tool { tool: &'static str, args: &'static [&'static str] },
    /// OCR the image, then translate the extracted text (translate-shell).
    OcrTranslate,
}

/// A vision provider.
#[derive(Clone, Copy, Debug)]
pub struct Provider {
    /// Short command name (`vision use <name>`).
    pub name: &'static str,
    pub label: &'static str,
    pub summary: &'static str,
    pub kind: ProviderKind,
}

impl Provider {
    /// External tool this provider needs (if any) — used by `slate doctor`.
    pub fn required_tool(&self) -> Option<&'static str> {
        match self.kind {
            ProviderKind::Tool { tool, .. } => Some(tool),
            ProviderKind::OcrTranslate => Some("tesseract"),
            ProviderKind::ByUrl { .. } => None,
        }
    }

    /// For `ProviderKind::Tool`: build argv with `{file}` replaced.
    pub fn tool_argv(&self, file: &str) -> Option<(String, Vec<String>)> {
        match self.kind {
            ProviderKind::Tool { tool, args } => Some((
                tool.to_string(),
                args.iter().map(|a| a.replace("{file}", file)).collect(),
            )),
            _ => None,
        }
    }

    /// For `ProviderKind::ByUrl`: build the provider page URL.
    pub fn url_for(&self, image_url: &str) -> Option<String> {
        match self.kind {
            ProviderKind::ByUrl { build } => Some(build(image_url)),
            _ => None,
        }
    }
}

/// Google Lens URL for a **publicly reachable** image URL.
pub fn lens_url(image_url: &str) -> String {
    format!("https://lens.google.com/uploadbyurl?url={}", percent_encode(image_url))
}

/// Bing Visual Search URL.
pub fn bing_url(image_url: &str) -> String {
    format!(
        "https://www.bing.com/images/search?view=detailv2&iss=sbi&q=imgurl:{}",
        percent_encode(image_url)
    )
}

/// Yandex Images URL.
pub fn yandex_url(image_url: &str) -> String {
    format!("https://yandex.com/images/search?rpt=imageview&url={}", percent_encode(image_url))
}

/// All built-in providers, in display order.
pub const PROVIDERS: &[Provider] = &[
    Provider {
        name: "lens",
        label: "Google Lens",
        summary: "reverse image search / Lens on a public image URL (opens browser)",
        kind: ProviderKind::ByUrl { build: lens_url },
    },
    Provider {
        name: "bing",
        label: "Bing Visual Search",
        summary: "Microsoft visual search on a public image URL (opens browser)",
        kind: ProviderKind::ByUrl { build: bing_url },
    },
    Provider {
        name: "yandex",
        label: "Yandex Images",
        summary: "Yandex reverse image search (opens browser)",
        kind: ProviderKind::ByUrl { build: yandex_url },
    },
    Provider {
        name: "ocr",
        label: "OCR (tesseract)",
        summary: "extract text from an image file",
        kind: ProviderKind::Tool { tool: "tesseract", args: &["{file}", "stdout"] },
    },
    Provider {
        name: "qr",
        label: "QR/barcode (zbarimg)",
        summary: "scan QR codes and barcodes from an image file",
        kind: ProviderKind::Tool { tool: "zbarimg", args: &["--quiet", "{file}"] },
    },
    Provider {
        name: "translate",
        label: "OCR + translate",
        summary: "OCR an image, then translate via translate-shell (`trans`)",
        kind: ProviderKind::OcrTranslate,
    },
];

/// Look up a provider by short name.
pub fn provider(name: &str) -> Option<&'static Provider> {
    PROVIDERS.iter().find(|p| p.name == name)
}

/// All provider short names.
pub fn names() -> Vec<&'static str> {
    PROVIDERS.iter().map(|p| p.name).collect()
}

/// The active vision configuration.
#[derive(Clone, Debug)]
pub struct VisionRegistry {
    default: String,
}

impl Default for VisionRegistry {
    fn default() -> Self {
        VisionRegistry { default: "lens".to_string() }
    }
}

impl VisionRegistry {
    pub fn with_default(name: impl Into<String>) -> Self {
        let name = name.into();
        let default = if provider(&name).is_some() { name } else { "lens".to_string() };
        VisionRegistry { default }
    }

    pub fn default_name(&self) -> &str {
        &self.default
    }

    pub fn default_provider(&self) -> &'static Provider {
        provider(&self.default).unwrap_or(&PROVIDERS[0])
    }

    pub fn set_default(&mut self, name: &str) -> bool {
        if provider(name).is_some() {
            self.default = name.to_string();
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urls_are_built() {
        let url = lens_url("https://example.com/a b.png");
        assert!(url.starts_with("https://lens.google.com/uploadbyurl?url="));
        assert!(url.contains("a%20b.png"));
        assert!(bing_url("https://x/y.png").contains("imgurl:"));
        assert!(yandex_url("https://x/y.png").contains("rpt=imageview"));
    }

    #[test]
    fn registry_defaults_and_switching() {
        let mut reg = VisionRegistry::default();
        assert_eq!(reg.default_name(), "lens");
        assert!(reg.set_default("ocr"));
        assert_eq!(reg.default_provider().name, "ocr");
        assert!(!reg.set_default("unknown"));
        assert_eq!(reg.default_name(), "ocr");
        assert_eq!(VisionRegistry::with_default("unknown").default_name(), "lens");
    }

    #[test]
    fn tool_expansion() {
        let ocr = provider("ocr").unwrap();
        let argv = ocr.tool_argv("/tmp/pic.png").unwrap();
        assert_eq!(argv, ("tesseract".to_string(), vec!["/tmp/pic.png".to_string(), "stdout".to_string()]));
        assert_eq!(ocr.required_tool(), Some("tesseract"));
        assert!(provider("lens").unwrap().url_for("https://x").is_some());
        assert!(provider("lens").unwrap().tool_argv("f").is_none());
    }
}
