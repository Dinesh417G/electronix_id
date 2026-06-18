use chrono::{DateTime, Utc};

use crate::domain::ids::OrgId;

#[derive(Debug, Clone)]
pub struct Organization {
    pub id: OrgId,
    pub name: String,
    pub slug: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Organization {
    /// Derive a URL-safe slug from a display name: lowercase, non-alphanumeric
    /// runs collapsed to single hyphens, trimmed.
    pub fn slugify(name: &str) -> String {
        let mut slug = String::with_capacity(name.len());
        let mut prev_hyphen = false;
        for ch in name.trim().chars() {
            if ch.is_ascii_alphanumeric() {
                slug.push(ch.to_ascii_lowercase());
                prev_hyphen = false;
            } else if !prev_hyphen {
                slug.push('-');
                prev_hyphen = true;
            }
        }
        slug.trim_matches('-').to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugify_collapses_and_trims() {
        assert_eq!(
            Organization::slugify("  Acme  Robotics!! "),
            "acme-robotics"
        );
        assert_eq!(Organization::slugify("ABB / KUKA"), "abb-kuka");
        assert_eq!(Organization::slugify("foo_bar"), "foo-bar");
    }
}
