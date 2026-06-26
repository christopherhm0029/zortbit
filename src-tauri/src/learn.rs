// Behaviour-learning engine.
//
// Instead of hand-written rules, Zortbit reads the user's REAL folder structure,
// fingerprints what already lives in each folder, and places a new file into the
// folder it most resembles. 100% local. Everything here is read-only discovery;
// the proposal/move layer is the only thing that writes.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

// Filename/content noise that carries no filing signal.
const STOP: &[&str] = &[
    "the", "and", "for", "with", "you", "your", "this", "that", "from", "into", "are", "was",
    "doc", "docx", "pdf", "ppt", "pptx", "png", "jpg", "jpeg", "heic", "webp", "gif", "html",
    "txt", "csv", "xls", "xlsx", "final", "draft", "copy", "version", "premium", "new", "old",
    "latest", "updated", "edited", "screenshot", "screen", "shot", "img", "image", "images",
    "photo", "cropped", "crop", "export", "untitled", "file", "files", "document", "documents",
    "scan", "scanned", "download", "downloads",
];

fn is_stop(t: &str) -> bool {
    STOP.contains(&t)
}

// Split a string into lowercase alphanumeric tokens, dropping noise & numbers.
pub fn tokenize(s: &str, out: &mut HashMap<String, u32>) {
    for raw in s.split(|c: char| !c.is_ascii_alphanumeric()) {
        if raw.len() < 3 || raw.len() > 24 {
            continue;
        }
        let t = raw.to_ascii_lowercase();
        if t.bytes().all(|b| b.is_ascii_digit()) || is_stop(&t) {
            continue;
        }
        *out.entry(t).or_insert(0) += 1;
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Folder {
    pub name: String,
    pub source_path: String,
    pub file_count: u32,
    pub tokens: HashMap<String, u32>, // token -> weight (the fingerprint)
}

pub struct Match {
    pub category: String,
    pub confidence: u8,
    pub shared: Vec<String>, // the discriminative tokens that drove the match
}

// A folder that looks like a code project / dump, not a filing destination.
const REPO_MARKERS: [&str; 9] = [
    ".git", "package.json", "Cargo.toml", "node_modules", "Pods", "target", ".venv", "build",
    "Package.swift",
];

fn is_code_repo(p: &Path) -> bool {
    REPO_MARKERS.iter().any(|m| p.join(m).exists())
}

fn is_doc_like(ext: &str) -> bool {
    matches!(
        ext,
        "pdf" | "doc" | "docx" | "rtf" | "odt" | "pages" | "ppt" | "pptx" | "key" | "txt" | "md"
            | "csv" | "xls" | "xlsx" | "numbers" | "png" | "jpg" | "jpeg" | "heic" | "webp"
            | "gif" | "tiff" | "svg" | "html"
    )
}

fn prune_top(m: &mut HashMap<String, u32>, n: usize) {
    if m.len() <= n {
        return;
    }
    let mut v: Vec<(String, u32)> = m.drain().collect();
    v.sort_by(|a, b| b.1.cmp(&a.1));
    v.truncate(n);
    *m = v.into_iter().collect();
}

// Fingerprint one folder from its filenames (+ a light content sample).
fn fingerprint(name: &str, dir: &Path) -> Option<Folder> {
    let mut tokens: HashMap<String, u32> = HashMap::new();
    // The folder name itself is the strongest self-signal — weight it up.
    let mut name_toks = HashMap::new();
    tokenize(name, &mut name_toks);
    for (t, _) in name_toks {
        *tokens.entry(t).or_insert(0) += 3;
    }

    let mut doc_files = 0u32;
    let mut total = 0u32;
    let mut sampled = 0u32;
    let mut stack = vec![(dir.to_path_buf(), 0u32)];
    while let Some((d, depth)) = stack.pop() {
        let rd = match std::fs::read_dir(&d) {
            Ok(r) => r,
            Err(_) => continue,
        };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                let bn = p.file_name().and_then(|s| s.to_str()).unwrap_or("");
                if depth < 1 && !bn.starts_with('.') && bn != "node_modules" && !is_code_repo(&p) {
                    stack.push((p, depth + 1));
                }
                continue;
            }
            total += 1;
            if total > 4000 {
                return None; // a dump/repo, not a filing bucket
            }
            let fname = match p.file_name().and_then(|s| s.to_str()) {
                Some(n) => n,
                None => continue,
            };
            if fname.starts_with('.') {
                continue;
            }
            let ext = p
                .extension()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            if !is_doc_like(&ext) {
                continue;
            }
            doc_files += 1;
            if let Some(stem) = p.file_stem().and_then(|s| s.to_str()) {
                tokenize(stem, &mut tokens);
            }
            // Sample real content from a few text docs (cheap, bounded).
            if sampled < 3 && matches!(ext.as_str(), "txt" | "md" | "csv" | "pdf" | "docx" | "doc" | "rtf" | "pptx") {
                if let Some(c) = crate::engine::extract_text(&p, &ext) {
                    tokenize(&c, &mut tokens);
                    sampled += 1;
                }
            }
        }
    }
    if doc_files < 2 {
        return None; // too little to characterize a destination
    }
    prune_top(&mut tokens, 48);
    Some(Folder {
        name: name.to_string(),
        source_path: dir.display().to_string(),
        file_count: doc_files,
        tokens,
    })
}

// Discover the user's real filing destinations across the configured roots.
// Skips code repos, dumps, and anything on the protected list.
pub fn learn_folders(home: &Path, roots: &[String], protected: &[PathBuf]) -> Vec<Folder> {
    let mut out: Vec<Folder> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    for rel in roots {
        let root = home.join(rel);
        let rd = match std::fs::read_dir(&root) {
            Ok(r) => r,
            Err(_) => continue,
        };
        for e in rd.flatten() {
            let p = e.path();
            if !p.is_dir() {
                continue;
            }
            let name = match p.file_name().and_then(|s| s.to_str()) {
                Some(n) => n.to_string(),
                None => continue,
            };
            if name.starts_with('.') || seen.contains(&name) {
                continue;
            }
            if protected.iter().any(|pp| pp == &p) || is_code_repo(&p) {
                continue;
            }
            if let Some(f) = fingerprint(&name, &p) {
                seen.insert(name.clone());
                out.push(f);
            }
        }
    }
    out
}

// Score a new file's tokens against every fingerprint. IDF-weights tokens so
// ones shared by many folders count for little; the most distinctive overlap wins.
pub fn best_match(
    file_tokens: &HashMap<String, u32>,
    name_tokens: &HashSet<String>,
    folders: &[Folder],
) -> Option<Match> {
    if folders.is_empty() || file_tokens.is_empty() {
        return None;
    }
    let nf = folders.len() as f64;
    let mut df: HashMap<&str, u32> = HashMap::new();
    for f in folders {
        for t in f.tokens.keys() {
            *df.entry(t.as_str()).or_insert(0) += 1;
        }
    }
    let idf = |t: &str| -> f64 {
        let d = *df.get(t).unwrap_or(&1) as f64;
        (((nf + 1.0) / (d + 0.5)).ln()).max(0.0)
    };

    let mut scored: Vec<(f64, &Folder, Vec<(String, f64)>)> = folders
        .iter()
        .map(|f| {
            let mut shared: HashMap<String, f64> = HashMap::new();
            // Distinctive content/filename overlap — rare tokens score higher.
            for t in file_tokens.keys() {
                if f.tokens.contains_key(t) {
                    let w = idf(t);
                    if w > 0.0 {
                        let e = shared.entry(t.clone()).or_insert(0.0);
                        if w > *e {
                            *e = w;
                        }
                    }
                }
            }
            // Decisive: the file is NAMED after this folder's project — the name
            // token is in the *filename*, not just buried in content. A file called
            // "Joblar_*.pptx" belongs in Joblar even if its contents resemble a
            // sibling folder where stray Joblar files were dropped. A folder name
            // that only appears inside content (an employer named in a CV) gets no
            // such bonus, so it can't hijack the file.
            let mut name_toks = HashMap::new();
            tokenize(&f.name, &mut name_toks);
            for nt in name_toks.keys() {
                if name_tokens.contains(nt) {
                    *shared.entry(nt.clone()).or_insert(0.0) += 8.0;
                }
            }
            let s: f64 = shared.values().sum();
            let mut sh: Vec<(String, f64)> = shared.into_iter().collect();
            sh.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            (s, f, sh)
        })
        .collect();
    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

    let first = scored.first()?;
    if first.0 <= 0.0 {
        return None;
    }
    let top_score = first.0;
    let second = scored.get(1).map(|x| x.0).unwrap_or(0.0);
    let margin = if second > 0.0 {
        top_score / (top_score + second)
    } else {
        1.0
    };
    let base = (top_score * 14.0).min(52.0);
    let conf = (40.0 + base + (margin - 0.5) * 56.0).round().clamp(35.0, 96.0) as u8;

    let mut sh = first.2.clone();
    sh.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    let shared: Vec<String> = sh.into_iter().take(3).map(|(t, _)| t).collect();
    Some(Match {
        category: first.1.name.clone(),
        confidence: conf,
        shared,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn folder(name: &str, toks: &[&str]) -> Folder {
        let mut t = HashMap::new();
        for x in toks {
            *t.entry((*x).to_string()).or_insert(0) += 1;
        }
        Folder {
            name: name.into(),
            source_path: String::new(),
            file_count: 5,
            tokens: t,
        }
    }

    #[test]
    fn places_by_distinctive_overlap_not_keyword() {
        // 'joblar'/'construction' are borrowed into Xaviour-AI (Joblar decks
        // physically live there), so a Joblar deck's CONTENT resembles Xaviour-AI.
        let folders = vec![
            folder("Joblar", &["joblar", "construction", "matching", "platform"]),
            folder("Xaviour-AI", &["xaviour", "joblar", "construction", "market", "laura", "global"]),
            folder("Personal-Docs", &["passport", "resume", "aws", "enterprise", "technical", "microsoft"]),
            folder("Microsoft", &["azure", "copilot", "github", "microsoft", "agent"]),
            folder("Learning", &["course", "exam", "microsoft", "azure"]),
        ];

        // A file NAMED Joblar → Joblar, even though its pitch CONTENT leans Xaviour-AI.
        let mut jb = HashMap::new();
        tokenize("joblar ndrc deck", &mut jb);
        let jb_name: HashSet<String> = jb.keys().cloned().collect();
        tokenize("global market laura pitch construction", &mut jb);
        assert_eq!(best_match(&jb, &jb_name, &folders).unwrap().category, "Joblar");

        // A Xaviour file → Xaviour-AI.
        let mut xv = HashMap::new();
        tokenize("xaviour v2 prototype", &mut xv);
        let xv_name: HashSet<String> = xv.keys().cloned().collect();
        assert_eq!(best_match(&xv, &xv_name, &folders).unwrap().category, "Xaviour-AI");

        // CV: filename names no project; 'microsoft' is only in its CONTENT, so it
        // must NOT hijack the file. Distinctive 'aws'/'enterprise' → Personal-Docs.
        let mut cv = HashMap::new();
        tokenize("christopher herrera aws senior solutions architect", &mut cv);
        let cv_name: HashSet<String> = cv.keys().cloned().collect();
        tokenize("microsoft enterprise cloud customer", &mut cv);
        assert_eq!(best_match(&cv, &cv_name, &folders).unwrap().category, "Personal-Docs");

        // No overlap → no forced match (caller falls back to the model/type).
        let mut noise = HashMap::new();
        tokenize("zzz qqq vvv", &mut noise);
        let noise_name: HashSet<String> = noise.keys().cloned().collect();
        assert!(best_match(&noise, &noise_name, &folders).is_none());
    }
}
