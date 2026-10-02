//! Where to send the browser after signing in: only ever a path on this site.

use percent_encoding::percent_decode_str;

/// `return_to` if it's a safe path on this site, otherwise `/`.
///
/// A safe path starts with a single `/`, has no backslashes, whitespace or
/// control characters, and stays that way once percent-decoded, so nothing
/// that decodes it again can turn it into another site.
pub fn sanitize(return_to: Option<&str>) -> String {
    match return_to {
        Some(path)
            if is_safe(path)
                && percent_decode_str(path).decode_utf8().is_ok_and(|d| is_safe(&d)) =>
        {
            path.to_owned()
        }
        _ => "/".to_owned(),
    }
}

fn is_safe(path: &str) -> bool {
    path.starts_with('/')
        && !path.starts_with("//")
        && !path.contains('\\')
        && !path.chars().any(|c| c.is_control() || c.is_whitespace())
        && path.len() <= 2048
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_site_paths_are_kept() {
        assert_eq!(sanitize(Some("/dev/blocks?card=3")), "/dev/blocks?card=3");
        assert_eq!(sanitize(Some("/")), "/");
        assert_eq!(sanitize(Some("/a%20b")), "/");
        assert_eq!(sanitize(Some("/caf%C3%A9")), "/caf%C3%A9");
    }

    #[test]
    fn anything_else_goes_home() {
        for path in [
            "https://evil.example/",
            "//evil.example",
            "/\\evil.example",
            "/\t/evil.example",
            "javascript:alert(1)",
            "%2F%2Fevil.example",
            "/%2F/evil.example",
            "/%5Cevil.example",
            "/%2F%2Fevil.example",
            "",
        ] {
            assert_eq!(sanitize(Some(path)), "/", "{path:?}");
        }
        assert_eq!(sanitize(None), "/");
    }
}
