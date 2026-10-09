//! Reading and writing questions as Markdown topic files.
//!
//! Each topic is one file (`rust.md`). A question is a level-2 heading followed
//! directly by a metadata comment; everything after that, up to the next
//! question, is the answer:
//!
//! ```markdown
//! # Rust
//!
//! ## Why does assigning a String move it?
//! <!-- id: mh2k3l9x4f  status: answered  tags: ownership, memory  asked: 2026-10-22 -->
//!
//! Because `String` owns heap data...
//! ```
//!
//! A `##` heading without the metadata comment directly below it is ordinary
//! answer content, and headings inside fenced code blocks are never questions.

use std::collections::HashMap;
use std::fs;
use std::hash::{Hash, Hasher};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

/// Topic used for questions typed without any hashtag.
pub const INBOX: &str = "inbox";
pub const ASSETS_DIR: &str = "assets";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Open,
    Answered,
}

impl Status {
    fn as_str(self) -> &'static str {
        match self {
            Status::Open => "open",
            Status::Answered => "answered",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Question {
    pub id: String,
    /// File stem of the topic file this question lives in.
    pub topic: String,
    pub title: String,
    pub status: Status,
    pub tags: Vec<String>,
    /// Date the question was asked, `YYYY-MM-DD`.
    pub asked: String,
    /// Raw Markdown answer; empty when unanswered.
    pub answer: String,
    /// Text captured with "Use selection", stored as a `> [!quote]` callout; empty if none.
    pub quote: String,
    /// Anything after `[!quote]` on the callout line (e.g. a title added in Obsidian), kept on rewrite.
    #[serde(skip)]
    pub quote_header: String,
}

/// One parsed topic file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TopicFile {
    pub topic: String,
    /// Everything before the first question (normally just `# Title`), kept verbatim.
    pub preamble: String,
    pub questions: Vec<Question>,
}

impl TopicFile {
    pub fn new(topic: &str) -> Self {
        TopicFile { topic: topic.to_string(), preamble: default_preamble(topic), questions: Vec::new() }
    }

    pub fn parse(topic: &str, text: &str) -> Self {
        let lines: Vec<&str> = text.lines().collect();
        let mut fence = Fence::default();
        let mut preamble = Vec::new();
        let mut questions: Vec<Question> = Vec::new();
        let mut answer: Vec<&str> = Vec::new();

        let mut i = 0;
        while i < lines.len() {
            let line = lines[i];
            if !fence.is_open() {
                if let Some(mut q) = question_at(topic, &lines, i) {
                    if let Some(prev) = questions.last_mut() {
                        prev.answer = join_answer(&answer);
                    }
                    answer.clear();
                    i += 2;
                    i += read_quote(&lines[i..], &mut q);
                    questions.push(q);
                    continue;
                }
            }
            fence.feed(line);
            if questions.is_empty() {
                preamble.push(line);
            } else {
                answer.push(line);
            }
            i += 1;
        }
        if let Some(prev) = questions.last_mut() {
            prev.answer = join_answer(&answer);
        }

        let preamble = preamble.join("\n").trim_end().to_string();
        TopicFile { topic: topic.to_string(), preamble, questions }
    }

    pub fn to_markdown(&self) -> String {
        let mut out = String::new();
        if !self.preamble.is_empty() {
            out.push_str(&self.preamble);
            out.push_str("\n\n");
        }
        for q in &self.questions {
            out.push_str("## ");
            out.push_str(&q.title);
            out.push('\n');
            out.push_str("<!-- id: ");
            out.push_str(&q.id);
            out.push_str("  status: ");
            out.push_str(q.status.as_str());
            if !q.tags.is_empty() {
                out.push_str("  tags: ");
                out.push_str(&q.tags.join(", "));
            }
            out.push_str("  asked: ");
            out.push_str(&q.asked);
            out.push_str(" -->\n");
            if !q.quote.is_empty() {
                out.push_str("> [!quote]");
                out.push_str(&q.quote_header);
                out.push('\n');
                for line in q.quote.lines() {
                    out.push('>');
                    if !line.is_empty() {
                        out.push(' ');
                        out.push_str(line);
                    }
                    out.push('\n');
                }
            }
            if !q.answer.is_empty() {
                out.push('\n');
                out.push_str(&q.answer);
                out.push('\n');
            }
            out.push('\n');
        }
        let trimmed = out.trim_end().len();
        out.truncate(trimmed);
        out.push('\n');
        out
    }

    /// True when the file holds nothing the user wrote, so it can be deleted.
    fn is_disposable(&self) -> bool {
        self.questions.is_empty() && self.preamble.trim() == default_preamble(&self.topic)
    }
}

fn default_preamble(topic: &str) -> String {
    let mut chars = topic.chars();
    let title: String = match chars.next() {
        Some(c) => c.to_uppercase().chain(chars).collect(),
        None => String::new(),
    };
    format!("# {}", title.replace(['-', '_'], " "))
}

/// Tracks whether we're inside a fenced code block (``` or ~~~).
#[derive(Default)]
struct Fence {
    open: Option<(char, usize)>,
}

impl Fence {
    fn is_open(&self) -> bool {
        self.open.is_some()
    }

    fn feed(&mut self, line: &str) {
        let t = line.trim_start();
        if line.len() - t.len() > 3 {
            return; // indented code, not a fence
        }
        let Some(c) = t.chars().next().filter(|c| *c == '`' || *c == '~') else { return };
        let run = t.chars().take_while(|x| *x == c).count();
        if run < 3 {
            return;
        }
        match self.open {
            None => self.open = Some((c, run)),
            Some((oc, olen)) if oc == c && run >= olen && t[run..].trim().is_empty() => self.open = None,
            Some(_) => {}
        }
    }
}

/// Parses a question header at `lines[i]` (heading + metadata comment on the next line).
fn question_at(topic: &str, lines: &[&str], i: usize) -> Option<Question> {
    let title = lines[i].strip_prefix("## ")?.trim();
    let meta = lines.get(i + 1)?.trim().strip_prefix("<!--")?.strip_suffix("-->")?;
    let fields = parse_meta(meta);
    let id = fields.get("id")?.to_string();
    if id.is_empty() {
        return None;
    }
    let status = match fields.get("status").copied() {
        Some("answered") => Status::Answered,
        _ => Status::Open,
    };
    let tags = fields
        .get("tags")
        .map(|t| t.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect())
        .unwrap_or_default();
    Some(Question {
        id,
        topic: topic.to_string(),
        title: title.to_string(),
        status,
        tags,
        asked: fields.get("asked").map(|s| s.to_string()).unwrap_or_default(),
        answer: String::new(),
        quote: String::new(),
        quote_header: String::new(),
    })
}

/// Reads a `> [!quote]` callout directly below the metadata line into `q`.
/// Returns how many lines it used (0 if there is no quote).
fn read_quote(lines: &[&str], q: &mut Question) -> usize {
    let Some(first) = lines.first().and_then(|l| l.trim_start().strip_prefix('>')) else { return 0 };
    let first = first.trim_start();
    if !first.to_lowercase().starts_with("[!quote]") {
        return 0;
    }
    q.quote_header = first["[!quote]".len()..].trim_end().to_string();
    let body: Vec<&str> = lines[1..]
        .iter()
        .map_while(|l| l.trim_start().strip_prefix('>'))
        .map(|l| l.strip_prefix(' ').unwrap_or(l))
        .collect();
    q.quote = normalize_quote(&body.join("\n"));
    1 + body.len()
}

/// Tidies captured text: unix newlines, no trailing spaces, no blank lines at either end.
fn normalize_quote(text: &str) -> String {
    let lines: Vec<&str> = text.lines().map(str::trim_end).collect();
    lines.join("\n").trim().to_string()
}

/// Splits `id: x  status: y  tags: a, b  asked: d` into its known keys.
fn parse_meta(meta: &str) -> HashMap<&'static str, &str> {
    const KEYS: [&str; 4] = ["id", "status", "tags", "asked"];
    let mut starts: Vec<(usize, usize, &'static str)> = Vec::new();
    for key in KEYS {
        let pat = format!("{key}:");
        let mut from = 0;
        while let Some(pos) = meta[from..].find(&pat) {
            let at = from + pos;
            let boundary = at == 0 || meta[..at].ends_with(char::is_whitespace);
            if boundary {
                starts.push((at, at + pat.len(), key));
                break;
            }
            from = at + pat.len();
        }
    }
    starts.sort();
    let mut out = HashMap::new();
    for (n, (_, value_start, key)) in starts.iter().enumerate() {
        let end = starts.get(n + 1).map(|s| s.0).unwrap_or(meta.len());
        out.insert(*key, meta[*value_start..end].trim());
    }
    out
}

fn join_answer(lines: &[&str]) -> String {
    let start = lines.iter().position(|l| !l.trim().is_empty()).unwrap_or(lines.len());
    lines[start..].join("\n").trim_end().to_string()
}

/// A question as typed in the quick-add box: `Why is X? #topic #tag1 #tag2`.
#[derive(Debug, PartialEq, Eq)]
pub struct Entry {
    pub title: String,
    /// First trailing hashtag, if any.
    pub topic: Option<String>,
    pub tags: Vec<String>,
}

/// Splits trailing hashtags off a typed question. Only hashtags at the end
/// count, so `C#` or `#include` in the middle of a sentence stay in the title.
pub fn parse_entry(text: &str) -> Entry {
    let mut title = text.trim();
    let mut hashtags = Vec::new();
    loop {
        let start = title
            .char_indices()
            .rev()
            .find(|(_, c)| c.is_whitespace())
            .map(|(p, c)| p + c.len_utf8())
            .unwrap_or(0);
        let token = &title[start..];
        match hashtag(token) {
            Some(tag) if start > 0 => {
                hashtags.push(tag);
                title = title[..start].trim_end();
            }
            _ => break,
        }
    }
    hashtags.reverse();
    let mut seen = Vec::new();
    for tag in hashtags {
        if !seen.contains(&tag) {
            seen.push(tag);
        }
    }
    let mut iter = seen.into_iter();
    let topic = iter.next();
    Entry { title: title.split_whitespace().collect::<Vec<_>>().join(" "), topic, tags: iter.collect() }
}

/// `#Rust-Lang` -> `rust-lang`; returns None for anything that isn't a valid hashtag.
fn hashtag(token: &str) -> Option<String> {
    let name = token.strip_prefix('#')?;
    let valid = !name.is_empty()
        && name.chars().all(|c| c.is_alphanumeric() || matches!(c, '-' | '_' | '+'))
        && !name.starts_with(['-', '+']);
    valid.then(|| name.to_lowercase())
}

/// Short, sortable, unique id: base-36 milliseconds plus a per-process counter.
fn new_id() -> String {
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let millis = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0) as u64;
    let n = COUNTER.fetch_add(1, Ordering::Relaxed) % (36 * 36);
    format!("{}{}", base36(millis), format!("{:0>2}", base36(n as u64)))
}

fn base36(mut n: u64) -> String {
    const DIGITS: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    if n == 0 {
        return "0".into();
    }
    let mut out = Vec::new();
    while n > 0 {
        out.push(DIGITS[(n % 36) as usize]);
        n /= 36;
    }
    out.reverse();
    String::from_utf8(out).unwrap()
}

fn today() -> String {
    chrono::Local::now().format("%Y-%m-%d").to_string()
}

pub fn content_hash(bytes: &[u8]) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    bytes.hash(&mut h);
    h.finish()
}

/// Hashes of what the app itself last wrote to each file, so the file watcher
/// can tell our own writes apart from external edits. `None` means deleted.
pub type OwnWrites = Arc<Mutex<HashMap<PathBuf, Option<u64>>>>;

#[derive(Debug)]
pub enum StoreError {
    NotFound(String),
    EmptyTitle,
    Io(io::Error),
}

impl std::fmt::Display for StoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StoreError::NotFound(id) => write!(f, "question {id} not found (was it edited outside the app?)"),
            StoreError::EmptyTitle => write!(f, "the question is empty"),
            StoreError::Io(e) => write!(f, "{e}"),
        }
    }
}

impl From<io::Error> for StoreError {
    fn from(e: io::Error) -> Self {
        StoreError::Io(e)
    }
}

pub type Result<T> = std::result::Result<T, StoreError>;

/// All questions in a notes directory. Files are re-read on every operation,
/// so edits made in other editors are always picked up.
pub struct Store {
    dir: PathBuf,
    own_writes: OwnWrites,
}

impl Store {
    pub fn new(dir: PathBuf) -> io::Result<Self> {
        fs::create_dir_all(&dir)?;
        Ok(Store { dir, own_writes: OwnWrites::default() })
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn own_writes(&self) -> OwnWrites {
        self.own_writes.clone()
    }

    fn path(&self, topic: &str) -> PathBuf {
        self.dir.join(format!("{topic}.md"))
    }

    /// Topic stems of every `.md` file in the notes directory.
    fn topics(&self) -> io::Result<Vec<String>> {
        let mut topics = Vec::new();
        for entry in fs::read_dir(&self.dir)? {
            let path = entry?.path();
            if path.extension().and_then(|e| e.to_str()) != Some("md") {
                continue;
            }
            if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                if !stem.starts_with('.') {
                    topics.push(stem.to_string());
                }
            }
        }
        topics.sort();
        Ok(topics)
    }

    fn load(&self, topic: &str) -> io::Result<TopicFile> {
        match fs::read_to_string(self.path(topic)) {
            Ok(text) => Ok(TopicFile::parse(topic, &text)),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(TopicFile::new(topic)),
            Err(e) => Err(e),
        }
    }

    /// Writes atomically (temp file + rename) so a crash never leaves a half-written file.
    fn save(&self, file: &TopicFile) -> io::Result<()> {
        let path = self.path(&file.topic);
        let mut writes = self.own_writes.lock().unwrap();
        if file.is_disposable() {
            writes.insert(path.clone(), None);
            return match fs::remove_file(&path) {
                Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
                _ => Ok(()),
            };
        }
        let text = file.to_markdown();
        let tmp = self.dir.join(format!(".{}.md.tmp", file.topic));
        fs::write(&tmp, &text)?;
        writes.insert(path.clone(), Some(content_hash(text.as_bytes())));
        fs::rename(&tmp, &path)
    }

    pub fn all(&self) -> io::Result<Vec<Question>> {
        let mut all = Vec::new();
        for topic in self.topics()? {
            all.extend(self.load(&topic)?.questions);
        }
        Ok(all)
    }

    fn find(&self, id: &str) -> Result<(TopicFile, usize)> {
        for topic in self.topics()? {
            let file = self.load(&topic)?;
            if let Some(idx) = file.questions.iter().position(|q| q.id == id) {
                return Ok((file, idx));
            }
        }
        Err(StoreError::NotFound(id.to_string()))
    }

    /// Adds a question typed as `title #topic #tags`, with an optional captured quote.
    pub fn add(&self, text: &str, quote: &str) -> Result<Question> {
        let entry = parse_entry(text);
        if entry.title.is_empty() {
            return Err(StoreError::EmptyTitle);
        }
        let topic = entry.topic.unwrap_or_else(|| INBOX.to_string());
        let q = Question {
            id: new_id(),
            topic: topic.clone(),
            title: entry.title,
            status: Status::Open,
            tags: entry.tags,
            asked: today(),
            answer: String::new(),
            quote: normalize_quote(quote),
            quote_header: String::new(),
        };
        let mut file = self.load(&topic)?;
        file.questions.push(q.clone());
        self.save(&file)?;
        Ok(q)
    }

    /// Saves an answer. The first time an answer is written, the question is
    /// marked answered; after that the status is only changed by `set_status`.
    pub fn set_answer(&self, id: &str, answer: &str) -> Result<Question> {
        let (mut file, idx) = self.find(id)?;
        let q = &mut file.questions[idx];
        let answer = answer.trim_end().to_string();
        if q.answer == answer {
            return Ok(q.clone());
        }
        if q.answer.is_empty() && !answer.is_empty() {
            q.status = Status::Answered;
        }
        q.answer = answer;
        let q = q.clone();
        self.save(&file)?;
        Ok(q)
    }

    pub fn set_status(&self, id: &str, status: Status) -> Result<Question> {
        let (mut file, idx) = self.find(id)?;
        file.questions[idx].status = status;
        let q = file.questions[idx].clone();
        self.save(&file)?;
        Ok(q)
    }

    /// Re-parses an edited `title #topic #tags` line. Without hashtags the topic
    /// is kept and the tags are cleared; a new topic moves the question to that file.
    /// `quote`, when given, replaces the captured quote (empty removes it).
    pub fn edit(&self, id: &str, text: &str, quote: Option<&str>) -> Result<Question> {
        let entry = parse_entry(text);
        if entry.title.is_empty() {
            return Err(StoreError::EmptyTitle);
        }
        let (mut file, idx) = self.find(id)?;
        let mut q = file.questions[idx].clone();
        q.title = entry.title;
        q.tags = entry.tags;
        if let Some(quote) = quote {
            q.quote = normalize_quote(quote);
        }
        let new_topic = entry.topic.unwrap_or_else(|| q.topic.clone());
        if new_topic == q.topic {
            file.questions[idx] = q.clone();
            self.save(&file)?;
        } else {
            q.topic = new_topic;
            // Write the destination first: a crash in between duplicates rather than loses.
            let mut dest = self.load(&q.topic)?;
            dest.questions.push(q.clone());
            self.save(&dest)?;
            file.questions.remove(idx);
            self.save(&file)?;
        }
        Ok(q)
    }

    pub fn delete(&self, id: &str) -> Result<()> {
        let (mut file, idx) = self.find(id)?;
        file.questions.remove(idx);
        self.save(&file)?;
        Ok(())
    }

    /// Stores image bytes under `assets/` and returns the path to use in Markdown.
    pub fn save_asset(&self, bytes: &[u8], ext: &str) -> io::Result<String> {
        let ext: String = ext.chars().filter(|c| c.is_ascii_alphanumeric()).take(5).collect();
        let ext = if ext.is_empty() { "png".to_string() } else { ext.to_lowercase() };
        let dir = self.dir.join(ASSETS_DIR);
        fs::create_dir_all(&dir)?;
        let name = format!("{}.{ext}", new_id());
        fs::write(dir.join(&name), bytes)?;
        Ok(format!("{ASSETS_DIR}/{name}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "# Rust

## Why does assigning a String move it?
<!-- id: a1  status: answered  tags: ownership, memory  asked: 2026-10-22 -->

Because `String` owns heap data.

### Example

```rust
let a = String::from(\"hi\");
## not a question
<!-- id: fake  status: open  asked: 2026-10-22 -->
let b = a;
```

## Just a heading in the answer

More text.

## What is a lifetime, really?
<!-- id: a2  status: open  asked: 2026-10-23 -->
";

    #[test]
    fn parses_questions_and_answers() {
        let f = TopicFile::parse("rust", SAMPLE);
        assert_eq!(f.preamble, "# Rust");
        assert_eq!(f.questions.len(), 2);
        let q = &f.questions[0];
        assert_eq!(q.id, "a1");
        assert_eq!(q.title, "Why does assigning a String move it?");
        assert_eq!(q.status, Status::Answered);
        assert_eq!(q.tags, vec!["ownership", "memory"]);
        assert_eq!(q.asked, "2026-10-22");
        assert!(q.answer.starts_with("Because `String`"));
        assert!(q.answer.contains("## not a question"), "fenced heading stays in the answer");
        assert!(q.answer.ends_with("More text."), "plain ## heading stays in the answer");
        let q2 = &f.questions[1];
        assert_eq!((q2.id.as_str(), q2.status, q2.answer.as_str()), ("a2", Status::Open, ""));
        assert!(q2.tags.is_empty());
    }

    #[test]
    fn canonical_round_trip_is_identity() {
        let f = TopicFile::parse("rust", SAMPLE);
        assert_eq!(f.to_markdown(), SAMPLE);
        assert_eq!(TopicFile::parse("rust", &f.to_markdown()), f);
    }

    #[test]
    fn tolerates_missing_and_reordered_meta_fields() {
        let text = "## Q\n<!--asked: 2026-01-01 id: x9 -->\nans\n";
        let f = TopicFile::parse("t", text);
        assert_eq!(f.preamble, "");
        let q = &f.questions[0];
        assert_eq!((q.id.as_str(), q.status, q.asked.as_str(), q.answer.as_str()), ("x9", Status::Open, "2026-01-01", "ans"));
    }

    #[test]
    fn tilde_fences_and_longer_fences() {
        let text = "## Q\n<!-- id: a -->\n````\n```\n## H\n<!-- id: b -->\n```\n````\n~~~\n## H2\n<!-- id: c -->\n~~~\n";
        let f = TopicFile::parse("t", text);
        assert_eq!(f.questions.len(), 1);
    }

    #[test]
    fn entry_parsing() {
        let e = parse_entry("Why does a String move?  #Rust #ownership #memory");
        assert_eq!(e, Entry { title: "Why does a String move?".into(), topic: Some("rust".into()), tags: vec!["ownership".into(), "memory".into()] });
        let e = parse_entry("What does #include do in C#?");
        assert_eq!((e.title.as_str(), e.topic), ("What does #include do in C#?", None));
        let e = parse_entry("dup #a #b #a");
        assert_eq!((e.topic, e.tags), (Some("a".into()), vec!["b".into()]));
        assert_eq!(parse_entry("#only").title, "#only", "a lone hashtag is the title, not a topic");
        assert_eq!(parse_entry("path #../etc").topic, None, "no path characters in topics");
    }

    fn temp_store() -> Store {
        let dir = std::env::temp_dir().join(format!("qb-test-{}", new_id()));
        Store::new(dir).unwrap()
    }

    #[test]
    fn store_add_answer_status_edit_move_delete() {
        let s = temp_store();
        let q = s.add("What is a monad? #haskell #fp", "").unwrap();
        assert_eq!((q.topic.as_str(), q.status), ("haskell", Status::Open));
        let text = fs::read_to_string(s.dir().join("haskell.md")).unwrap();
        assert!(text.starts_with("# Haskell\n\n## What is a monad?\n<!-- id: "));

        let q = s.set_answer(&q.id, "A monoid in the category of endofunctors.\n\n").unwrap();
        assert_eq!(q.status, Status::Answered, "first answer marks it answered");
        let q = s.set_status(&q.id, Status::Open).unwrap();
        let q = s.set_answer(&q.id, "Changed.").unwrap();
        assert_eq!(q.status, Status::Open, "manual override sticks after later edits");

        let inbox = s.add("loose question", "").unwrap();
        assert_eq!(inbox.topic, INBOX);

        let moved = s.edit(&q.id, "What is a monad, really? #category-theory #fp", None).unwrap();
        assert_eq!(moved.topic, "category-theory");
        assert!(!s.dir().join("haskell.md").exists(), "emptied auto-created topic file is removed");
        let all = s.all().unwrap();
        assert_eq!(all.len(), 2);
        let m = all.iter().find(|x| x.id == q.id).unwrap();
        assert_eq!((m.title.as_str(), m.answer.as_str()), ("What is a monad, really?", "Changed."));

        let kept = s.edit(&q.id, "Title only", None).unwrap();
        assert_eq!((kept.topic.as_str(), kept.tags.len()), ("category-theory", 0));

        s.delete(&inbox.id).unwrap();
        assert!(matches!(s.delete(&inbox.id), Err(StoreError::NotFound(_))));
        assert_eq!(s.all().unwrap().len(), 1);
        fs::remove_dir_all(s.dir()).unwrap();
    }

    #[test]
    fn hand_written_topic_file_is_kept_when_emptied() {
        let s = temp_store();
        fs::write(s.dir().join("notes.md"), "# My notes\n\nSome intro I wrote.\n").unwrap();
        let q = s.add("q #notes", "").unwrap();
        s.delete(&q.id).unwrap();
        assert_eq!(fs::read_to_string(s.dir().join("notes.md")).unwrap(), "# My notes\n\nSome intro I wrote.\n");
        fs::remove_dir_all(s.dir()).unwrap();
    }

    const QUOTED: &str = "## Why does the borrow end here?
<!-- id: b1  status: open  asked: 2026-10-22 -->
> [!quote]
> A reference's scope starts where it is introduced
>
> and ends at its last use.

My answer.
> a normal blockquote in the answer

## Plain blockquote answer
<!-- id: b2  status: open  asked: 2026-10-22 -->

> not a captured quote
";

    #[test]
    fn parses_quote_callouts() {
        let f = TopicFile::parse("rust", QUOTED);
        let q = &f.questions[0];
        assert_eq!(q.quote, "A reference's scope starts where it is introduced\n\nand ends at its last use.");
        assert_eq!(q.answer, "My answer.\n> a normal blockquote in the answer");
        let q2 = &f.questions[1];
        assert_eq!((q2.quote.as_str(), q2.answer.as_str()), ("", "> not a captured quote"));
        assert_eq!(f.to_markdown(), QUOTED, "quotes round-trip unchanged");
    }

    #[test]
    fn keeps_obsidian_callout_title() {
        let text = "## Q\n<!-- id: c1  status: open  asked: 2026-10-22 -->\n> [!QUOTE] Rust Book ch.4\n> text\n";
        let f = TopicFile::parse("t", text);
        assert_eq!(f.questions[0].quote, "text");
        assert!(f.to_markdown().contains("> [!quote] Rust Book ch.4\n> text\n"));
    }

    #[test]
    fn store_saves_and_edits_quotes() {
        let s = temp_store();
        let q = s.add("Why? #rust", "  first line   \r\nsecond\n\n").unwrap();
        assert_eq!(q.quote, "first line\nsecond");
        let all = s.all().unwrap();
        assert_eq!(all[0].quote, "first line\nsecond");
        let q = s.edit(&q.id, "Why? #rust", Some("")).unwrap();
        assert_eq!(q.quote, "");
        assert!(!fs::read_to_string(s.dir().join("rust.md")).unwrap().contains("[!quote]"));
        fs::remove_dir_all(s.dir()).unwrap();
    }

    #[test]
    fn ids_are_unique() {
        let ids: std::collections::HashSet<_> = (0..500).map(|_| new_id()).collect();
        assert_eq!(ids.len(), 500);
    }
}
