//! Files by meaning (plans/ui-ideas/spec-later.md, R5-37): the desktop's search finds files in Documents by what they
//! are about, with the index's own embedding model (potion-base-8M), from the text GNOME's indexer extracted.
//! `/usr/libexec/amethystora-file-meaning index` hands each file's path, its modification time and the start of its
//! text over as JSON lines; `write` works out each one's vector and keeps, in the account's own folder
//! (~/.local/state/amethystora/file-meaning, 0700), only the paths and the vectors: files.jsonl and vectors.f16, both
//! 0600, never a word of the text. A file whose time did not change keeps the vector it had. `search` embeds the words
//! asked and answers the nearest files that are still there, above a floor, for the shell's search.

use std::collections::HashMap;
use std::io::BufRead;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::index::embed::{self, Model};

/// The model of the agent's index, in the image
pub const MODEL: &str = "/usr/share/amethystora/agents/knowledge/model";
/// A file closer than this to what was asked is said (cosine of two normalised means of token vectors)
pub const FLOOR: f32 = 0.42;
/// At most this many files, and this much of a line handed over
const MOST: usize = 20000;
const LONGEST_LINE: usize = 64 * 1024;

/// The folder the index is kept in, for this account
pub fn dir() -> PathBuf {
    let state = std::env::var_os("XDG_STATE_HOME").map(PathBuf::from).filter(|path| path.is_absolute());
    let state = state.unwrap_or_else(|| PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".local/state"));
    state.join("amethystora/file-meaning")
}

struct Entry {
    path: String,
    modified: String,
    vector: Vec<f32>,
}

/// What the folder holds now: each file's path, time and vector, or nothing when its two files do not agree
fn load(dir: &Path) -> (Vec<Entry>, usize) {
    let Ok(text) = std::fs::read_to_string(dir.join("files.jsonl")) else { return (Vec::new(), 0) };
    let Ok(bytes) = std::fs::read(dir.join("vectors.f16")) else { return (Vec::new(), 0) };
    let rows: Vec<(String, String)> = text
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter_map(|value| Some((value["path"].as_str()?.to_string(), value["modified"].as_str().unwrap_or("").to_string())))
        .collect();
    if rows.is_empty() || bytes.len() % (rows.len() * 2) != 0 {
        return (Vec::new(), 0);
    }
    let dims = bytes.len() / rows.len() / 2;
    let entries = rows
        .into_iter()
        .enumerate()
        .map(|(at, (path, modified))| Entry {
            path,
            modified,
            vector: bytes[at * dims * 2..(at + 1) * dims * 2].chunks_exact(2).map(|b| embed::from_f16(u16::from_le_bytes([b[0], b[1]]))).collect(),
        })
        .collect();
    (entries, dims)
}

/// A file of the folder, written whole under another name and then put in place, 0600 from the start
fn put(dir: &Path, name: &str, bytes: &[u8]) -> Result<(), String> {
    let fresh = dir.join(format!(".{name}.new"));
    let _ = std::fs::remove_file(&fresh);
    {
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&fresh)
            .map_err(|error| format!("{}: {error}", fresh.display()))?;
        file.write_all(bytes).map_err(|error| error.to_string())?;
    }
    std::fs::rename(&fresh, dir.join(name)).map_err(|error| error.to_string())
}

/// Reads the JSON lines {path, modified, text} and writes the index into `dir`; says how many files it holds
pub fn write(dir: &Path, model: &Model, input: impl BufRead) -> Result<usize, String> {
    std::fs::create_dir_all(dir).map_err(|error| format!("{}: {error}", dir.display()))?;
    std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700)).map_err(|error| error.to_string())?;
    let (old, old_dims) = load(dir);
    let mut kept: HashMap<(String, String), Vec<f32>> = HashMap::new();
    if old_dims == model.dims {
        for entry in old {
            kept.insert((entry.path, entry.modified), entry.vector);
        }
    }
    let mut lines = String::new();
    let mut vectors: Vec<u8> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut count = 0;
    for line in input.lines() {
        let Ok(line) = line else { break };
        if line.len() > LONGEST_LINE || count >= MOST {
            continue;
        }
        let Ok(value) = serde_json::from_str::<Value>(&line) else { continue };
        let (Some(path), Some(text)) = (value["path"].as_str(), value["text"].as_str()) else { continue };
        let modified = value["modified"].as_str().unwrap_or("").to_string();
        if !path.starts_with('/') || path.contains('\n') || !seen.insert(path.to_string()) {
            continue;
        }
        let vector = match kept.remove(&(path.to_string(), modified.clone())) {
            Some(vector) => vector,
            None => {
                let file_name = Path::new(path).file_stem().map(|stem| stem.to_string_lossy().replace(['-', '_', '.'], " ")).unwrap_or_default();
                model.embed(&format!("{file_name}\n{text}"))
            }
        };
        // A file with no word the model knows has no meaning to compare
        if vector.iter().all(|value| *value == 0.0) {
            continue;
        }
        lines.push_str(&json!({"path": path, "modified": modified}).to_string());
        lines.push('\n');
        for value in &vector {
            vectors.extend_from_slice(&embed::to_f16(*value).to_le_bytes());
        }
        count += 1;
    }
    // The vectors first: a reader that finds the two out of step answers nothing until the next reading
    put(dir, "vectors.f16", &vectors)?;
    put(dir, "files.jsonl", lines.as_bytes())?;
    Ok(count)
}

/// The files nearest to `words`, at most `k`, each still a file: [{path, score}]
pub fn search(dir: &Path, model: &Model, words: &str, k: usize) -> Vec<Value> {
    let (entries, dims) = load(dir);
    if entries.is_empty() || dims != model.dims {
        return Vec::new();
    }
    let query = model.embed(words);
    if query.iter().all(|value| *value == 0.0) {
        return Vec::new();
    }
    let mut scored: Vec<(f32, &Entry)> = entries.iter().map(|entry| (embed::dot(&query, &entry.vector), entry)).filter(|(score, _)| *score >= FLOOR).collect();
    scored.sort_by(|a, b| b.0.total_cmp(&a.0));
    scored
        .into_iter()
        .filter(|(_, entry)| Path::new(&entry.path).is_file())
        .take(k)
        .map(|(score, entry)| json!({"path": entry.path, "score": (score * 1000.0).round() / 1000.0}))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A model of four words in two directions, so that "refund" and "tax" mean the same and "garden" does not
    fn model(dir: &Path) -> Model {
        let vocab = ["[PAD]", "[UNK]", "refund", "tax", "garden", "rose"];
        let tokenizer = json!({
            "normalizer": {"type": "BertNormalizer", "lowercase": true, "strip_accents": null},
            "pre_tokenizer": {"type": "BertPreTokenizer"},
            "model": {"type": "WordPiece", "unk_token": "[UNK]", "continuing_subword_prefix": "##", "max_input_chars_per_word": 100,
                "vocab": vocab.iter().enumerate().map(|(i, t)| (t.to_string(), json!(i))).collect::<serde_json::Map<String, Value>>()}
        });
        std::fs::write(dir.join("tokenizer.json"), tokenizer.to_string()).unwrap();
        std::fs::write(dir.join("config.json"), "{\"normalize\": true}").unwrap();
        let rows: [[f32; 2]; 6] = [[0.0, 0.0], [0.0, 0.0], [1.0, 0.1], [0.9, 0.0], [0.0, 1.0], [0.1, 0.9]];
        let data: Vec<u8> = rows.iter().flatten().flat_map(|v| v.to_le_bytes()).collect();
        let header = json!({"embeddings": {"dtype": "F32", "shape": [6, 2], "data_offsets": [0, data.len()]}}).to_string();
        let mut bytes = (header.len() as u64).to_le_bytes().to_vec();
        bytes.extend_from_slice(header.as_bytes());
        bytes.extend_from_slice(&data);
        std::fs::write(dir.join("model.safetensors"), bytes).unwrap();
        Model::load(dir).unwrap()
    }

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("amethystora-files-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn found_by_meaning_kept_as_paths_and_vectors_only() {
        let root = scratch("meaning");
        let model = model(&root);
        let docs = root.join("Documents");
        std::fs::create_dir_all(&docs).unwrap();
        for name in ["letter.txt", "plants.txt"] {
            std::fs::write(docs.join(name), "x").unwrap();
        }
        let input = format!(
            "{}\n{}\nnot json\n{}\n",
            json!({"path": docs.join("letter.txt"), "modified": "1", "text": "the refund came"}),
            json!({"path": docs.join("plants.txt"), "modified": "1", "text": "a rose garden"}),
            json!({"path": docs.join("gone.txt"), "modified": "1", "text": "tax"}),
        );
        let index = root.join("index");
        assert_eq!(write(&index, &model, input.as_bytes()).unwrap(), 3);
        let found = search(&index, &model, "tax", 5);
        // The letter, by meaning (no word in common); not the garden; not the file that is gone
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(found[0]["path"].as_str().unwrap().ends_with("letter.txt"));
        let kept = std::fs::read_to_string(index.join("files.jsonl")).unwrap();
        assert!(!kept.contains("refund") && !kept.contains("rose"));
        assert_eq!(std::fs::metadata(index.join("files.jsonl")).unwrap().permissions().mode() & 0o777, 0o600);
        assert_eq!(std::fs::metadata(&index).unwrap().permissions().mode() & 0o777, 0o700);
        // Read again with the same times, the vectors are kept, whatever the text says now
        let again = json!({"path": docs.join("letter.txt"), "modified": "1", "text": "rose rose"}).to_string();
        write(&index, &model, again.as_bytes()).unwrap();
        assert_eq!(search(&index, &model, "refund", 5).len(), 1);
        // ...and a changed time reads the text again
        let changed = json!({"path": docs.join("letter.txt"), "modified": "2", "text": "rose rose"}).to_string();
        write(&index, &model, changed.as_bytes()).unwrap();
        assert!(search(&index, &model, "refund", 5).is_empty());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn nothing_from_an_index_out_of_step() {
        let root = scratch("step");
        let model = model(&root);
        let index = root.join("index");
        std::fs::create_dir_all(&index).unwrap();
        std::fs::write(index.join("files.jsonl"), "{\"path\": \"/a\"}\n{\"path\": \"/b\"}\n").unwrap();
        std::fs::write(index.join("vectors.f16"), [0u8; 6]).unwrap();
        assert!(search(&index, &model, "tax", 5).is_empty());
        let _ = std::fs::remove_dir_all(&root);
    }
}
