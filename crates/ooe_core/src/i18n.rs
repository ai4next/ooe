//! Japanese and Korean catalogs (`i18n/*.tsv`, keyed by the English text).
//! English and Chinese are inline in the code; see `oo_base::i18n`.

use std::sync::Once;

use oo_base::Lang;

pub const JA: &str = include_str!("../../../i18n/ja.tsv");
pub const KO: &str = include_str!("../../../i18n/ko.tsv");

/// Installs the catalogs once. Call at startup and at the top of tests that
/// read translated text.
pub fn install() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        oo_base::i18n::install(Lang::Ja, JA).expect("i18n/ja.tsv is malformed");
        oo_base::i18n::install(Lang::Ko, KO).expect("i18n/ko.tsv is malformed");
    });
}

#[cfg(test)]
mod tests {
    #[test]
    fn catalogs_parse() {
        assert!(oo_base::i18n::parse(super::JA).is_ok());
        assert!(oo_base::i18n::parse(super::KO).is_ok());
        super::install();
    }
}
