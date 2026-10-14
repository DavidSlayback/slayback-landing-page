//! The colour schemes shared with the homepage hero (`site/hero.js`), and which one a page starts in.

/// A scheme: four neon accents in sRGB, the first one primary.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Scheme {
    pub key: &'static str,
    pub accents: [[f32; 3]; 4],
}

pub const SCHEMES: [Scheme; 3] = [
    Scheme { key: "netrunner", accents: [[0.62, 1.0, 0.0], [0.15, 1.0, 0.2], [0.0, 1.0, 0.72], [0.88, 1.0, 0.12]] },
    Scheme { key: "synthwave", accents: [[1.0, 0.08, 0.72], [0.0, 0.9, 1.0], [1.0, 0.92, 0.05], [0.5, 0.15, 1.0]] },
    Scheme { key: "gold", accents: [[1.0, 0.55, 0.04], [1.0, 0.82, 0.2], [1.0, 0.3, 0.0], [1.0, 0.95, 0.65]] },
];

/// Index in [`SCHEMES`] of the scheme named `key`.
pub fn index(key: &str) -> Option<usize> {
    SCHEMES.iter().position(|s| s.key == key)
}

/// The page's starting scheme: `?palette=` in the page's `query` if given, else the visitor's `saved` pick (the
/// hero's `slayback-scheme` in localStorage); Netrunner when neither names a scheme.
pub fn page_choice(query: &str, saved: Option<&str>) -> usize {
    let asked = query.trim_start_matches('?').split('&').find_map(|kv| kv.strip_prefix("palette="));
    asked.or(saved).and_then(index).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_query_beats_the_saved_pick() {
        assert_eq!(page_choice("?palette=gold", Some("synthwave")), 2);
        assert_eq!(page_choice("?demo=1&palette=synthwave", None), 1);
        assert_eq!(page_choice("", Some("synthwave")), 1);
        assert_eq!(page_choice("?palette=plaid", Some("gold")), 0, "an unknown name in the query is the default");
        assert_eq!(page_choice("", Some("plaid")), 0);
        assert_eq!(page_choice("", None), 0);
    }
}
