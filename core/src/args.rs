//! Minimal argument vector helpers shared by the binaries.
//!
//! CLIs stay hand-rolled (the workspace has no `clap` dependency); these
//! helpers keep flag parsing consistent and testable.

/// Remove and return whether any of `names` (e.g. `["-h", "--help"]`) was
/// present in `args`.
pub fn take_flag(args: &mut Vec<String>, names: &[&str]) -> bool {
    let mut found = false;
    let mut i = 0;
    while i < args.len() {
        if names.contains(&args[i].as_str()) {
            args.remove(i);
            found = true;
        } else {
            i += 1;
        }
    }
    found
}

/// Remove and return the value of an option in either `--opt value` or
/// `--opt=value` form.
pub fn take_option(args: &mut Vec<String>, names: &[&str]) -> Option<String> {
    let mut i = 0;
    while i < args.len() {
        let a = args[i].clone();
        for name in names {
            if a.as_str() == *name {
                if i + 1 < args.len() {
                    let v = args.remove(i + 1);
                    args.remove(i);
                    return Some(v);
                }
                return None; // flag without value; leave it for error reporting
            }
            if let Some(eq) = a.strip_prefix(&format!("{name}=")) {
                args.remove(i);
                return Some(eq.to_string());
            }
        }
        i += 1;
    }
    None
}

/// Join remaining positionals into one string (e.g. the rest of a command).
pub fn rest(args: &[String]) -> String {
    args.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(xs: &[&str]) -> Vec<String> {
        xs.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn flags_are_consumed() {
        let mut a = v(&["run", "--verbose", "echo hi"]);
        assert!(take_flag(&mut a, &["-v", "--verbose"]));
        assert!(!take_flag(&mut a, &["--dry-run"]));
        assert_eq!(a, v(&["run", "echo hi"]));
    }

    #[test]
    fn options_both_forms() {
        let mut a = v(&["--out", "x.svg"]);
        assert_eq!(take_option(&mut a, &["--out"]).as_deref(), Some("x.svg"));

        let mut b = v(&["--out=y.svg"]);
        assert_eq!(take_option(&mut b, &["--out"]).as_deref(), Some("y.svg"));

        let mut c = v(&["--out"]);
        assert_eq!(take_option(&mut c, &["--out"]), None); // missing value
    }

    #[test]
    fn rest_joins() {
        assert_eq!(rest(&v(&["a", "b", "c"])), "a b c");
        assert_eq!(rest(&[]), "");
    }
}
