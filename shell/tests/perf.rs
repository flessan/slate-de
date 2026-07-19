//! Performance budgets.
//!
//! The industry answer is `criterion` — but criterion contradicts Slate's
//! zero-external-dependency rule, so budgets live here as plain tests with
//! *generous* wall-clock limits (≈20–50× real-world timings in debug mode)
//! whose job is to catch algorithmic regressions, not to benchmark.
//!
//! Real numbers belong in `docs/PERFORMANCE.md`, refreshed per release via
//! `scripts/bench.sh`.

use std::time::{Duration, Instant};

use slate_command::{Ctx, Registry};
use slate_shell::{render_snapshot, App, SnapshotOptions, SnapshotView};

fn demo_ctx() -> Ctx {
    let mut ctx = Ctx::ephemeral();
    ctx.demo = true;
    ctx.headless = false;
    ctx
}

/// Hard limit helper with a readable failure message.
fn budget(what: &str, limit: Duration, f: impl FnOnce()) {
    let start = Instant::now();
    f();
    let elapsed = start.elapsed();
    assert!(
        elapsed <= limit,
        "{what} took {elapsed:?}, over the {limit:?} budget (see docs/PERFORMANCE.md)"
    );
}

#[test]
fn search_index_build_is_linear_enough() {
    let registry = Registry::builtins();
    let mut ctx = demo_ctx();
    // Plant a synthetic file tree: 40 dirs × 25 files = 1000 indexed files.
    let root = std::env::temp_dir().join(format!("slate-perf-files-{}", std::process::id()));
    for d in 0..40 {
        let dir = root.join(format!("dir{d}"));
        std::fs::create_dir_all(&dir).unwrap();
        for f in 0..25 {
            std::fs::write(dir.join(format!("file{f}.txt")), b"x").unwrap();
        }
    }
    ctx.cfg
        .doc
        .set(
            "search.roots",
            slate_config::Value::Array(vec![slate_config::Value::Str(
                root.to_string_lossy().into_owned(),
            )]),
        )
        .unwrap();
    ctx.cfg.doc.set("search.depth", slate_config::Value::Int(3)).unwrap();
    ctx.cfg.doc.set("search.max_entries", slate_config::Value::Int(5000)).unwrap();
    budget("refresh_search(1000 files)", Duration::from_secs(3), || {
        ctx.refresh_search(&registry);
    });
    assert!(ctx.index.len() > 900, "index should see the files: {}", ctx.index.len());
    // Queries at scale.
    budget("100 fuzzy queries over 1000+ items", Duration::from_secs(3), || {
        for i in 0..100 {
            let hits = ctx.index.query("file", 10);
            assert!(!hits.is_empty(), "query {i} should hit");
        }
    });
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn command_dispatch_is_fast() {
    let registry = Registry::builtins();
    let mut ctx = demo_ctx();
    budget("200 command executions", Duration::from_secs(4), || {
        for _ in 0..100 {
            registry.exec("theme list", &mut ctx).unwrap();
            registry.exec("help", &mut ctx).unwrap();
        }
    });
}

#[test]
fn frame_rendering_stays_interactive() {
    let mut app = App::new(demo_ctx(), Registry::builtins());
    let mut term = slate_term::test_terminal(160, 48);
    budget("100 frames at 160x48", Duration::from_secs(5), || {
        for _ in 0..100 {
            term.draw(|buf, area| app.draw_frame(buf, area)).unwrap();
        }
    });
}

#[test]
fn snapshot_pipeline_is_cheap() {
    budget("20 deterministic snapshots", Duration::from_secs(5), || {
        for _ in 0..20 {
            let svg = render_snapshot(&SnapshotOptions {
                view: SnapshotView::Desktop,
                ..Default::default()
            })
            .unwrap();
            assert!(svg.starts_with("<svg"));
        }
    });
}

#[test]
fn layout_engine_scales() {
    let spec = slate_core::layout::LayoutSpec::parse("(a:70 | (b / c)):80 / (d | e | f):20").unwrap();
    let area = slate_core::rect::Rect::new(0, 0, 200, 60);
    budget("10_000 layout computes", Duration::from_secs(2), || {
        for _ in 0..10_000 {
            let rects = spec.compute(area, 0);
            assert_eq!(rects.len(), 5);
        }
    });
}
