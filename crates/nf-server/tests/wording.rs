//! Work plan 3.3, rule A.3: the interface (web/), the package texts and the install texts never use approved,
//! certified, validated, production-ready, secure, hack-proof, error-free or AI-native as a claim about the product.
//!
//! Every occurrence of one of these words (any case; a hyphen may also be a space or nothing) must lie inside an exact
//! phrase of `ALLOWED`, each with the reason it is not a claim: honest negatives ("Not validated"), the name of the
//! state no result can reach (APPROVED), the certified error bound of the mathematics (a property of a number, stated
//! with its proof in docs/backend/TOLERANCE_POLICY.md section 8, and its negatives), and the name of the cookie
//! attribute `Secure`. The list holds phrases, never single words, and an entry no file uses any more fails the test,
//! so the list cannot grow stale or wide. A word inside an identifier (`cookie_secure_flag`) is not a word.
use std::path::PathBuf;

const WORDS: [&str; 8] = [
    "approved",
    "certified",
    "validated",
    "production-ready",
    "secure",
    "hack-proof",
    "error-free",
    "ai-native",
];

/// The files that are scanned, relative to the repository root: every file of the interface, the package texts and
/// the install texts (the installer and launcher scripts' messages, the server installation guide).
const FILES: [&str; 13] = [
    "web/index.html",
    "web/app.js",
    "web/i18n.js",
    "web/viewer.js",
    "web/styles.css",
    "packaging/README.txt",
    "packaging/INSTALL.md",
    "packaging/NOTICE.txt",
    "packaging/linux/nexus-fabric.desktop",
    "packaging/linux/nexus-fabric.sh",
    "packaging/windows/installer.nsi",
    "deploy/README.md",
    "deploy/install.sh",
];

/// (exact phrase, why it is not a claim about the product). Case-sensitive.
const ALLOWED: &[(&str, &str)] = &[
    (
        "not validated",
        "honest negative: the build is not validated (development-build notice)",
    ),
    (
        "Not validated",
        "honest negative: the build is not validated (development-build notice)",
    ),
    (
        "NOT VALIDATED",
        "honest negative: the status-bar form of the development-build notice",
    ),
    (
        "'state.APPROVED': 'Approved'",
        "label of the state no result can reach: engineer approval (gate G16) is not implemented",
    ),
    (
        "'state.APPROVED': '承認済み'",
        "Japanese label of the same unreachable state",
    ),
    (
        "{ok} approved'",
        "dashboard count of jobs in the unreachable state; always zero",
    ),
    (
        "'dash.l.ok': '{n} approved'",
        "dashboard count of jobs in the unreachable state; always zero",
    ),
    (
        "No job can reach APPROVED",
        "honest negative: names the unreachable state",
    ),
    (
        "no job can reach APPROVED",
        "honest negative: names the unreachable state",
    ),
    (
        "APPROVED: 'ok'",
        "the unreachable state's identifier in the client's code (status colour map)",
    ),
    (
        "['APPROVED', 'REJECTED', 'SUPERSEDED', 'NO_RESULT', 'REVIEW_REQUIRED', 'REFUSED']",
        "the state identifiers the client can label, in its code",
    ),
    (
        "ok: count('APPROVED')",
        "the client counts jobs in the unreachable state (always zero)",
    ),
    (
        "state !== 'APPROVED'",
        "the client's blocked-job rule names the unreachable state",
    ),
    (
        "interval-certified error bounds",
        "a planned capability listed as not implemented",
    ),
    (
        "not interval-certified",
        "honest negative about the numerical error bound eps_num",
    ),
    (
        "certified upper bound of the chord error",
        "a proved property of the number eps_rep (docs/backend/TOLERANCE_POLICY.md section 8), not of the product",
    ),
    (
        "eps_rep is a certified bound",
        "a proved property of the number eps_rep (TOLERANCE_POLICY section 8), not of the product",
    ),
    (
        "eps_rep is a certified upper bound (interval arithmetic",
        "the engine's sentence about the bound eps_rep, matched for translation",
    ),
    (
        "where the pattern states it certified",
        "the engine's sentence about the bound eps_rep, matched for translation",
    ),
    ("NOT certified", "honest negative about the bound eps_rep"),
    (
        "eps_assumption): a certified",
        "code comment about the bound eps_rep",
    ),
    (
        "closure cannot be certified",
        "honest negative: the server's sentence about scrutiny obligations, matched for translation",
    ),
    (
        "`Cookie`の`Secure`属性",
        "the name of the cookie attribute Secure (RFC 6265) on the security status page",
    ),
];

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

/// Byte ranges of every occurrence of a forbidden word in `text`, with the word as written.
fn occurrences(text: &str) -> Vec<(usize, usize, String)> {
    let lower = text.to_ascii_lowercase();
    let is_word = |c: Option<char>| c.is_some_and(|c| c.is_ascii_alphanumeric() || c == '_');
    let mut out = Vec::new();
    for w in WORDS {
        let mut forms = vec![w.to_string()];
        if w.contains('-') {
            forms.push(w.replace('-', " "));
            forms.push(w.replace('-', ""));
        }
        for f in forms {
            let mut from = 0;
            while let Some(i) = lower[from..].find(&f) {
                let (s, e) = (from + i, from + i + f.len());
                if !is_word(lower[..s].chars().next_back()) && !is_word(lower[e..].chars().next()) {
                    out.push((s, e, text[s..e].to_string()));
                }
                from = e;
            }
        }
    }
    out
}

/// The allowed phrases that cover the byte range s..e of `text`.
fn covering(text: &str, s: usize, e: usize) -> Vec<usize> {
    let mut hits = Vec::new();
    for (k, (phrase, _)) in ALLOWED.iter().enumerate() {
        let mut from = 0;
        while let Some(i) = text[from..].find(phrase) {
            let (ps, pe) = (from + i, from + i + phrase.len());
            if ps <= s && e <= pe {
                hits.push(k);
            }
            from = ps + 1;
            while !text.is_char_boundary(from) {
                from += 1;
            }
        }
    }
    hits
}

#[test]
fn no_forbidden_claim_in_the_interface_package_or_install_texts() {
    let mut bad = Vec::new();
    let mut used = vec![false; ALLOWED.len()];
    // the fixed files plus every module of the client under web/js, so a new screen is covered without editing this list
    let mut files: Vec<String> = FILES.iter().map(|f| f.to_string()).collect();
    let mut dirs = vec![root().join("web/js")];
    let mut modules = Vec::new();
    while let Some(d) = dirs.pop() {
        for e in std::fs::read_dir(&d).unwrap_or_else(|e| panic!("{}: {e}", d.display())) {
            let p = e.unwrap().path();
            if p.is_dir() {
                dirs.push(p);
            } else if p.extension().is_some_and(|x| x == "js") {
                modules.push(
                    p.strip_prefix(root())
                        .unwrap()
                        .to_string_lossy()
                        .replace('\\', "/"),
                );
            }
        }
    }
    modules.sort();
    files.extend(modules);
    for f in &files {
        let path = root().join(f);
        let text =
            std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        for (s, e, word) in occurrences(&text) {
            let hits = covering(&text, s, e);
            if hits.is_empty() {
                let line = text[..s].matches('\n').count() + 1;
                let a = text[..s].char_indices().rev().nth(50).map_or(0, |(i, _)| i);
                let b = text[e..]
                    .char_indices()
                    .nth(50)
                    .map_or(text.len(), |(i, _)| e + i);
                bad.push(format!("{f}:{line}: \"{word}\" in …{}…", &text[a..b]));
            }
            for k in hits {
                used[k] = true;
            }
        }
    }
    assert!(
        bad.is_empty(),
        "forbidden words (rule A.3) outside the allow-list:\n{}",
        bad.join("\n")
    );
    let stale: Vec<&str> = ALLOWED
        .iter()
        .zip(&used)
        .filter(|(_, u)| !**u)
        .map(|((p, _), _)| *p)
        .collect();
    assert!(
        stale.is_empty(),
        "allow-list entries no scanned file uses (remove them): {stale:?}"
    );
}

#[test]
fn the_allow_list_holds_phrases_not_words() {
    for (phrase, why) in ALLOWED {
        assert!(
            phrase.split_whitespace().count() >= 2 || phrase.contains('`') || phrase.contains('-'),
            "{phrase:?} is a single word, not a phrase"
        );
        assert!(!why.trim().is_empty(), "{phrase:?} has no reason");
        assert!(
            !occurrences(phrase).is_empty(),
            "{phrase:?} holds no forbidden word"
        );
    }
}

#[test]
fn the_check_finds_a_claim() {
    // the scanner itself: each word, in each form, is found; an identifier is not
    for claim in [
        "fully Validated output",
        "production ready",
        "Production-Ready",
        "productionready",
        "a secure program",
        "hack proof",
        "error-free",
        "AI-native design",
        "APPROVED for use",
        "certified.",
    ] {
        assert!(!occurrences(claim).is_empty(), "{claim}");
    }
    for not_a_word in [
        "cookie_secure_flag",
        "insecurely",
        "validatedness",
        "unapproved_x",
    ] {
        assert!(occurrences(not_a_word).is_empty(), "{not_a_word}");
    }
}
