use std::collections::HashMap;
use std::path::{Path, PathBuf};

use tracing::{debug, trace};
use unicode_width::UnicodeWidthChar;
use walkdir::WalkDir;

#[derive(Debug, Clone, Copy)]
pub enum BalloonMode {
    Say,
    Think,
}

#[derive(Debug, Clone)]
pub struct BalloonStyle {
    pub top_left: String,
    pub top_right: String,
    pub bottom_left: String,
    pub bottom_right: String,
    pub horizontal: String,
    pub first_left: String,
    pub first_right: String,
    pub middle_left: String,
    pub middle_right: String,
    pub last_left: String,
    pub last_right: String,
    pub think_left: String,
    pub think_right: String,
}

impl Default for BalloonStyle {
    fn default() -> Self {
        Self {
            top_left: "/".to_string(),
            top_right: "\\".to_string(),
            bottom_left: "\\".to_string(),
            bottom_right: "/".to_string(),
            horizontal: "-".to_string(),
            first_left: "/".to_string(),
            first_right: "\\".to_string(),
            middle_left: "|".to_string(),
            middle_right: "|".to_string(),
            last_left: "\\".to_string(),
            last_right: "/".to_string(),
            think_left: "(".to_string(),
            think_right: ")".to_string(),
        }
    }
}

pub fn load_style(name: &str, roots: &[PathBuf]) -> Option<BalloonStyle> {
    for root in roots {
        for candidate in style_candidates(root, name) {
            if candidate.is_file() {
                if let Ok(style) = parse_style_file(&candidate) {
                    debug!(path = %candidate.display(), "loaded balloon style");
                    return Some(style);
                }
            }
        }
    }
    None
}

pub fn render_balloon(
    message: &str,
    width: usize,
    mode: BalloonMode,
    style: &BalloonStyle,
) -> Vec<String> {
    let wrapped = wrap_message(message, width.max(1));
    let content_width = wrapped
        .iter()
        .map(|line| visible_width(line))
        .max()
        .unwrap_or(0);

    let mut rendered = Vec::new();
    rendered.push(format!(
        "{}{}{}",
        style.top_left,
        style.horizontal.repeat(content_width + 2),
        style.top_right
    ));

    if wrapped.len() == 1 {
        let line = pad_to_width(&wrapped[0], content_width);
        match mode {
            BalloonMode::Say => rendered.push(format!("< {} >", line)),
            BalloonMode::Think => rendered.push(format!(
                "{} {} {}",
                style.think_left, line, style.think_right
            )),
        }
    } else {
        for (index, line) in wrapped.iter().enumerate() {
            let line = pad_to_width(line, content_width);
            match mode {
                BalloonMode::Say => {
                    let (left, right) = if index == 0 {
                        (&style.first_left, &style.first_right)
                    } else if index + 1 == wrapped.len() {
                        (&style.last_left, &style.last_right)
                    } else {
                        (&style.middle_left, &style.middle_right)
                    };
                    rendered.push(format!("{} {} {}", left, line, right));
                }
                BalloonMode::Think => {
                    rendered.push(format!(
                        "{} {} {}",
                        style.think_left, line, style.think_right
                    ));
                }
            }
        }
    }

    rendered.push(format!(
        "{}{}{}",
        style.bottom_left,
        style.horizontal.repeat(content_width + 2),
        style.bottom_right
    ));

    rendered
}

fn style_candidates(root: &Path, name: &str) -> [PathBuf; 2] {
    [root.join(name), root.join(format!("{name}.balloon"))]
}

fn parse_style_file(path: &Path) -> Result<BalloonStyle, std::io::Error> {
    let raw = std::fs::read_to_string(path)?;
    let mut kv = HashMap::new();
    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if let Some((key, value)) = trimmed.split_once('=') {
            kv.insert(key.trim().to_string(), value.trim().to_string());
        }
    }

    let mut style = BalloonStyle::default();
    macro_rules! assign {
        ($field:ident, $key:literal) => {
            if let Some(value) = kv.get($key) {
                style.$field = value.to_string();
            }
        };
    }

    assign!(top_left, "top_left");
    assign!(top_right, "top_right");
    assign!(bottom_left, "bottom_left");
    assign!(bottom_right, "bottom_right");
    assign!(horizontal, "horizontal");
    assign!(first_left, "first_left");
    assign!(first_right, "first_right");
    assign!(middle_left, "middle_left");
    assign!(middle_right, "middle_right");
    assign!(last_left, "last_left");
    assign!(last_right, "last_right");
    assign!(think_left, "think_left");
    assign!(think_right, "think_right");

    Ok(style)
}

fn wrap_message(message: &str, width: usize) -> Vec<String> {
    let mut out = Vec::new();

    for line in message.lines() {
        if line.trim().is_empty() {
            out.push(String::new());
            continue;
        }

        let mut current = String::new();
        for word in line.split_whitespace() {
            let word_width = visible_width(word);
            if current.is_empty() {
                if word_width <= width {
                    current.push_str(word);
                    continue;
                }
                out.extend(hard_wrap(word, width));
                continue;
            }

            let projected = visible_width(&current) + 1 + word_width;
            if projected <= width {
                current.push(' ');
                current.push_str(word);
            } else {
                out.push(current);
                if word_width <= width {
                    current = word.to_string();
                } else {
                    out.extend(hard_wrap(word, width));
                    current = String::new();
                }
            }
        }

        if !current.is_empty() {
            out.push(current);
        }
    }

    if out.is_empty() {
        out.push(String::new());
    }

    out
}

fn hard_wrap(word: &str, width: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut current_width = 0;

    for ch in word.chars() {
        let w = UnicodeWidthChar::width(ch).unwrap_or(0);
        if current_width + w > width && !current.is_empty() {
            out.push(current);
            current = String::new();
            current_width = 0;
        }
        current.push(ch);
        current_width += w;
    }

    if !current.is_empty() {
        out.push(current);
    }

    out
}

fn pad_to_width(input: &str, width: usize) -> String {
    let mut out = input.to_string();
    let visible = visible_width(input);
    if visible < width {
        out.push_str(&" ".repeat(width - visible));
    }
    out
}

fn visible_width(input: &str) -> usize {
    let mut width = 0;
    let mut chars = input.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '\u{1b}' {
            if chars.peek() == Some(&'[') {
                chars.next();
                while let Some(next) = chars.next() {
                    if ('@'..='~').contains(&next) {
                        break;
                    }
                }
                continue;
            }
        }
        width += UnicodeWidthChar::width(ch).unwrap_or(0);
    }

    trace!(input = input, width = width, "computed visible width");
    width
}

pub fn list_balloon_names(root: &Path) -> Vec<String> {
    let mut names = Vec::new();

    if !root.exists() {
        return names;
    }

    for entry in WalkDir::new(root)
        .follow_links(false)
        .min_depth(1)
        .max_depth(2)
        .into_iter()
        .filter_map(Result::ok)
    {
        if !entry.file_type().is_file() {
            continue;
        }

        if let Some(name) = entry.path().file_stem().and_then(|name| name.to_str()) {
            names.push(name.to_string());
        }
    }

    names.sort();
    names.dedup();
    names
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wraps_lines() {
        let lines = wrap_message("a bb ccc dddd", 6);
        assert_eq!(lines, vec!["a bb", "ccc", "dddd"]);
    }

    #[test]
    fn renders_think_balloon() {
        let style = BalloonStyle::default();
        let lines = render_balloon("hello", 20, BalloonMode::Think, &style);
        assert!(lines.iter().any(|line| line.contains("( hello )")));
    }
}
