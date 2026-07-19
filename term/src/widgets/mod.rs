//! Pure-function widgets: every widget is `fn(Buffer, Rect, spec)`.
//!
//! Widgets own no state; panes keep state and pass it in. That keeps widgets
//! trivially testable and deterministic (the `slate snapshot` command renders
//! real UI through these exact code paths).

mod block;
mod gauge;
mod list;
mod paragraph;
mod sparkline;

pub use block::{block, BlockSpec, BorderKind, Borders};
pub use gauge::{gauge, GaugeSpec};
pub use list::{list, ListSpec};
pub use paragraph::{paragraph, wrap_line, ParagraphOpts};
pub use sparkline::{sparkline, SparklineSpec, SPARK_BARS};
