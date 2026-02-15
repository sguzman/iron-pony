use std::collections::HashMap;
use std::path::{Path, PathBuf};

use tracing::{debug, trace, warn};
use unicode_width::UnicodeWidthChar;
use walkdir::WalkDir;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BalloonMode {
    Say,
    Think,
}

#[derive(Debug, Clone)]
pub struct BalloonStyle {
    pub link: String,
    pub link_mirror: String,
    pub link_cross: String,
    pub ww: String,
    pub ee: String,
    pub nw: Vec<String>,
    pub nnw: Vec<String>,
    pub n: Vec<String>,
    pub nne: Vec<String>,
    pub ne: Vec<String>,
    pub nee: String,
    pub e: String,
    pub see: String,
    pub se: Vec<String>,
    pub sse: Vec<String>,
    pub s: Vec<String>,
    pub ssw: Vec<String>,
    pub sw: Vec<String>,
    pub sww: String,
    pub w: String,
    pub nww: String,
    pub min_width: usize,
    pub min_height: usize,
}

impl BalloonStyle {
    #[allow(clippy::too_many_arguments)]
    fn new(
        link: String,
        link_mirror: String,
        link_cross: String,
        ww: String,
        ee: String,
        nw: Vec<String>,
        nnw: Vec<String>,
        n: Vec<String>,
        nne: Vec<String>,
        ne: Vec<String>,
        nee: String,
        e: String,
        see: String,
        se: Vec<String>,
        sse: Vec<String>,
        s: Vec<String>,
        ssw: Vec<String>,
        sw: Vec<String>,
        sww: String,
        w: String,
        nww: String,
    ) -> Self {
        let ne_widest = ne
            .iter()
            .max_by_key(|value| visible_width(value))
            .cloned()
            .unwrap_or_default();
        let _nw_widest = nw
            .iter()
            .max_by_key(|value| visible_width(value))
            .cloned()
            .unwrap_or_default();
        let se_widest = se
            .iter()
            .max_by_key(|value| visible_width(value))
            .cloned()
            .unwrap_or_default();
        let _sw_widest = sw
            .iter()
            .max_by_key(|value| visible_width(value))
            .cloned()
            .unwrap_or_default();

        // Keep upstream's exact width math (including its historical oddities).
        let min_e = [
            ne_widest.as_str(),
            nee.as_str(),
            e.as_str(),
            see.as_str(),
            se_widest.as_str(),
            ee.as_str(),
        ]
        .iter()
        .map(|item| visible_width(item))
        .max()
        .unwrap_or(0);

        let min_n = [ne.len(), nne.len(), n.len(), nnw.len(), nw.len()]
            .into_iter()
            .max()
            .unwrap_or(0);
        let min_s = [se.len(), sse.len(), s.len(), ssw.len(), sw.len()]
            .into_iter()
            .max()
            .unwrap_or(0);

        Self {
            link,
            link_mirror,
            link_cross,
            ww,
            ee,
            nw,
            nnw,
            n,
            nne,
            ne,
            nee,
            e,
            see,
            se,
            sse,
            s,
            ssw,
            sw,
            sww,
            w,
            nww,
            min_width: min_e + min_e,
            min_height: min_n + min_s,
        }
    }

    fn default_for_mode(mode: BalloonMode) -> Self {
        match mode {
            BalloonMode::Think => Self::new(
                "o".to_string(),
                "o".to_string(),
                "o".to_string(),
                "( ".to_string(),
                " )".to_string(),
                vec![" _".to_string()],
                vec!["_".to_string()],
                vec!["_".to_string()],
                vec!["_".to_string()],
                vec!["_ ".to_string()],
                " )".to_string(),
                " )".to_string(),
                " )".to_string(),
                vec!["- ".to_string()],
                vec!["-".to_string()],
                vec!["-".to_string()],
                vec!["-".to_string()],
                vec![" -".to_string()],
                "( ".to_string(),
                "( ".to_string(),
                "( ".to_string(),
            ),
            BalloonMode::Say => Self::new(
                "\\".to_string(),
                "/".to_string(),
                "X".to_string(),
                "< ".to_string(),
                " >".to_string(),
                vec![" _".to_string()],
                vec!["_".to_string()],
                vec!["_".to_string()],
                vec!["_".to_string()],
                vec!["_ ".to_string()],
                " \\".to_string(),
                " |".to_string(),
                " /".to_string(),
                vec!["- ".to_string()],
                vec!["-".to_string()],
                vec!["-".to_string()],
                vec!["-".to_string()],
                vec![" -".to_string()],
                "\\ ".to_string(),
                "| ".to_string(),
                "/ ".to_string(),
            ),
        }
    }

    fn render(&self, minw: usize, minh: usize, lines: &[String]) -> Vec<String> {
        let mut h = self.min_height + lines.len();
        let mut w = self.min_width
            + lines
                .iter()
                .max_by_key(|line| visible_width(line))
                .map(|line| visible_width(line))
                .unwrap_or(0);

        if w < minw {
            w = minw;
        }
        if h < minh {
            h = minh;
        }

        let mut ws = HashMap::<usize, &str>::new();
        let mut es = HashMap::<usize, &str>::new();

        if lines.len() > 1 {
            ws.insert(0, self.nww.as_str());
            ws.insert(lines.len() - 1, self.sww.as_str());
            es.insert(0, self.nee.as_str());
            es.insert(lines.len() - 1, self.see.as_str());
            for j in 1..(lines.len() - 1) {
                ws.insert(j, self.w.as_str());
                es.insert(j, self.e.as_str());
            }
        } else {
            ws.insert(0, self.ww.as_str());
            es.insert(0, self.ee.as_str());
        }

        let mut rendered = Vec::new();

        for j in 0..self.n.len() {
            let nw = self.nw.get(j).map_or("", String::as_str);
            let ne = self.ne.get(j).map_or("", String::as_str);
            let nnw = self.nnw.get(j).map_or("", String::as_str);
            let nne = self.nne.get(j).map_or("", String::as_str);
            let n = self.n.get(j).map_or("", String::as_str);

            let outer = visible_width(nw) + visible_width(ne);
            let inner = visible_width(nnw) + visible_width(nne);

            if outer + inner <= w {
                rendered.push(format!(
                    "{}{}{}{}{}",
                    nw,
                    nnw,
                    n.repeat(w - outer - inner),
                    nne,
                    ne
                ));
            } else {
                rendered.push(format!("{}{}{}", nw, n.repeat(w - outer), ne));
            }
        }

        for (index, line) in lines.iter().enumerate() {
            let left = ws.get(&index).copied().unwrap_or(self.w.as_str());
            let right = es.get(&index).copied().unwrap_or(self.e.as_str());
            let pad = w
                .saturating_sub(visible_width(line))
                .saturating_sub(visible_width(self.w.as_str()))
                .saturating_sub(visible_width(self.e.as_str()));
            rendered.push(format!("{}{}{}", left, line, " ".repeat(pad) + right));
        }

        for j in 0..self.s.len() {
            let sw = self.sw.get(j).map_or("", String::as_str);
            let se = self.se.get(j).map_or("", String::as_str);
            let ssw = self.ssw.get(j).map_or("", String::as_str);
            let sse = self.sse.get(j).map_or("", String::as_str);
            let s = self.s.get(j).map_or("", String::as_str);

            let outer = visible_width(sw) + visible_width(se);
            let inner = visible_width(ssw) + visible_width(sse);

            if outer + inner <= w {
                rendered.push(format!(
                    "{}{}{}{}{}",
                    sw,
                    ssw,
                    s.repeat(w - outer - inner),
                    sse,
                    se
                ));
            } else {
                rendered.push(format!("{}{}{}", sw, s.repeat(w - outer), se));
            }
        }

        trace!(
            width = w,
            height = h,
            lines = rendered.len(),
            "rendered balloon"
        );
        rendered
    }
}

pub fn load_style(
    name: Option<&str>,
    roots: &[PathBuf],
    mode: BalloonMode,
) -> Option<BalloonStyle> {
    let Some(name) = name else {
        return Some(BalloonStyle::default_for_mode(mode));
    };

    for candidate in style_candidates(name, roots, mode) {
        if !candidate.is_file() {
            continue;
        }

        match parse_style_file(&candidate) {
            Ok(style) => {
                debug!(path = %candidate.display(), "loaded balloon style");
                return Some(style);
            }
            Err(error) => {
                warn!(path = %candidate.display(), %error, "failed parsing balloon style");
            }
        }
    }

    None
}

pub fn render_balloon(message: &str, width: usize, style: &BalloonStyle) -> Vec<String> {
    let wrap_target = width.saturating_sub(style.min_width).max(1);
    let wrapped = wrap_message(message, wrap_target)
        .into_iter()
        .map(|line| format!("{line}\u{1b}[0m"))
        .collect::<Vec<_>>();
    let rendered = style.render(0, 0, &wrapped);

    rendered
        .into_iter()
        .map(|line| format!("\u{1b}[0m{}\u{1b}[0m", line))
        .collect()
}

fn style_candidates(name: &str, roots: &[PathBuf], mode: BalloonMode) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let name_path = PathBuf::from(name);

    if name_path.is_absolute() || name.contains('/') {
        out.push(name_path.clone());
    }

    let suffix = match mode {
        BalloonMode::Say => "say",
        BalloonMode::Think => "think",
    };

    for root in roots {
        out.push(root.join(name));
        out.push(root.join(format!("{name}.{suffix}")));
        out.push(root.join(format!("{name}.balloon")));
    }

    out
}

fn parse_style_file(path: &Path) -> Result<BalloonStyle, std::io::Error> {
    let raw = std::fs::read_to_string(path)?;

    let keys = [
        "\\", "/", "X", "ww", "ee", "nw", "nnw", "n", "nne", "ne", "nee", "e", "see", "se", "sse",
        "s", "ssw", "sw", "sww", "w", "nww",
    ];

    let mut map = HashMap::<String, Vec<String>>::new();
    for key in keys {
        map.insert(key.to_string(), Vec::new());
    }

    let mut last: Option<String> = None;

    for line in raw.lines() {
        let line = line.trim_end_matches('\r');
        if line.is_empty() {
            continue;
        }

        if let Some(rest) = line.strip_prefix(':') {
            if let Some(last_key) = &last {
                if let Some(values) = map.get_mut(last_key) {
                    values.push(rest.to_string());
                }
            }
            continue;
        }

        let Some((key, value)) = line.split_once(':') else {
            continue;
        };

        if let Some(values) = map.get_mut(key) {
            values.push(value.to_string());
            last = Some(key.to_string());
        }
    }

    fn one(map: &HashMap<String, Vec<String>>, key: &str) -> String {
        map.get(key)
            .and_then(|v| v.first())
            .cloned()
            .unwrap_or_default()
    }

    fn many(map: &HashMap<String, Vec<String>>, key: &str) -> Vec<String> {
        map.get(key).cloned().unwrap_or_default()
    }

    Ok(BalloonStyle::new(
        one(&map, "\\"),
        one(&map, "/"),
        one(&map, "X"),
        one(&map, "ww"),
        one(&map, "ee"),
        many(&map, "nw"),
        many(&map, "nnw"),
        many(&map, "n"),
        many(&map, "nne"),
        many(&map, "ne"),
        one(&map, "nee"),
        one(&map, "e"),
        one(&map, "see"),
        many(&map, "se"),
        many(&map, "sse"),
        many(&map, "s"),
        many(&map, "ssw"),
        many(&map, "sw"),
        one(&map, "sww"),
        one(&map, "w"),
        one(&map, "nww"),
    ))
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
                } else {
                    out.extend(hard_wrap(word, width));
                }
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

fn visible_width(input: &str) -> usize {
    let mut width = 0;
    let chars = input.chars().collect::<Vec<_>>();
    let mut i = 0;

    while i < chars.len() {
        let ch = chars[i];
        if ch == '\u{1b}' {
            i += consume_escape(&chars[i..]);
            continue;
        }

        i += 1;
        width += UnicodeWidthChar::width(ch).unwrap_or(0);
    }

    width
}

fn consume_escape(chars: &[char]) -> usize {
    if chars.is_empty() || chars[0] != '\u{1b}' {
        return 0;
    }
    if chars.len() == 1 {
        return 1;
    }

    let mut i = 1;
    let c = chars[i];
    i += 1;

    if c == ']' {
        while i < chars.len() {
            let ch = chars[i];
            i += 1;
            if ch == '\\' || ch == '\u{7}' {
                break;
            }
        }
        return i;
    }

    if c == '[' {
        while i < chars.len() {
            let ch = chars[i];
            i += 1;
            if ch == '~' || ch.is_ascii_alphabetic() {
                break;
            }
        }
        return i;
    }

    i
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
    fn wrap_splits_words() {
        let lines = wrap_message("a bb ccc dddd", 6);
        assert_eq!(lines, vec!["a bb", "ccc", "dddd"]);
    }

    #[test]
    fn parse_ascii_style() {
        let raw = "\\:\\\n/:/\nX:X\n\nn:_\n: \n";
        let file = tempfile::NamedTempFile::new().expect("tempfile");
        std::fs::write(file.path(), raw).expect("write");
        let style = parse_style_file(file.path()).expect("style");
        assert_eq!(style.link, "\\");
        assert_eq!(style.link_mirror, "/");
    }
}
