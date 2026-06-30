use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn render_markdown(source: &str) -> String {
    Renderer::new(source).render()
}

struct Renderer<'a> {
    lines: Vec<&'a str>,
    index: usize,
    html: String,
}

impl<'a> Renderer<'a> {
    fn new(source: &'a str) -> Self {
        Self {
            lines: source.lines().collect(),
            index: 0,
            html: String::new(),
        }
    }

    fn render(mut self) -> String {
        while self.index < self.lines.len() {
            let line = self.lines[self.index];
            let trimmed = line.trim();

            if trimmed.is_empty() {
                self.index += 1;
                continue;
            }

            if let Some(lang) = fenced_language(trimmed) {
                self.render_code_block(lang);
                continue;
            }

            if trimmed.starts_with("$$") {
                self.render_math_block();
                continue;
            }

            if let Some((level, content)) = heading(trimmed) {
                self.html.push_str(&format!(
                    "<h{level}>{}</h{level}>",
                    render_inline(content.trim())
                ));
                self.index += 1;
                continue;
            }

            if is_blockquote(trimmed) {
                self.render_blockquote();
                continue;
            }

            if list_item(trimmed).is_some() {
                self.render_list();
                continue;
            }

            self.render_paragraph();
        }

        self.html
    }

    fn render_code_block(&mut self, lang: &str) {
        self.index += 1;
        let mut code = String::new();

        while self.index < self.lines.len() {
            let line = self.lines[self.index];
            if line.trim_start().starts_with("```") {
                self.index += 1;
                break;
            }
            code.push_str(line);
            code.push('\n');
            self.index += 1;
        }

        let normalized_lang = normalize_language(lang);
        let highlighted = highlight_code(&code, normalized_lang);
        self.html.push_str(&format!(
            "<pre class=\"code-block language-{}\"><code>{}</code></pre>",
            escape_attr(normalized_lang),
            highlighted
        ));
    }

    fn render_math_block(&mut self) {
        let first = self.lines[self.index].trim();
        let mut math = String::new();

        if first.starts_with("$$") && first.ends_with("$$") && first.len() > 4 {
            math.push_str(first.trim_start_matches("$$").trim_end_matches("$$").trim());
            self.index += 1;
            self.html
                .push_str(&latex_to_mathml(math.trim(), MathDisplay::Block));
            return;
        }

        if first.len() > 2 {
            math.push_str(first.trim_start_matches("$$").trim());
        }
        self.index += 1;

        while self.index < self.lines.len() {
            let line = self.lines[self.index].trim();
            if line.ends_with("$$") {
                let content = line.trim_end_matches("$$").trim();
                if !content.is_empty() {
                    if !math.is_empty() {
                        math.push(' ');
                    }
                    math.push_str(content);
                }
                self.index += 1;
                break;
            }

            if !math.is_empty() {
                math.push(' ');
            }
            math.push_str(line);
            self.index += 1;
        }

        self.html
            .push_str(&latex_to_mathml(math.trim(), MathDisplay::Block));
    }

    fn render_blockquote(&mut self) {
        let mut content = String::new();

        while self.index < self.lines.len() {
            let trimmed = self.lines[self.index].trim();
            if !is_blockquote(trimmed) {
                break;
            }

            let text = trimmed.trim_start_matches('>').trim_start();
            if !content.is_empty() {
                content.push('\n');
            }
            content.push_str(text);
            self.index += 1;
        }

        let inner = Renderer::new(&content).render();
        self.html.push_str("<blockquote>");
        self.html.push_str(&inner);
        self.html.push_str("</blockquote>");
    }

    fn render_list(&mut self) {
        let ordered = ordered_list_item(self.lines[self.index].trim()).is_some();
        self.html.push_str(if ordered { "<ol>" } else { "<ul>" });

        while self.index < self.lines.len() {
            let trimmed = self.lines[self.index].trim();
            let item = if ordered {
                ordered_list_item(trimmed)
            } else {
                unordered_list_item(trimmed)
            };

            let Some(item) = item else {
                break;
            };

            self.html.push_str("<li>");
            if let Some((checked, text)) = task_list_marker(item) {
                let checked_attr = if checked { " checked" } else { "" };
                self.html.push_str(&format!(
                    "<input type=\"checkbox\" disabled{}> {}",
                    checked_attr,
                    render_inline(text)
                ));
            } else {
                self.html.push_str(&render_inline(item));
            }
            self.html.push_str("</li>");
            self.index += 1;
        }

        self.html.push_str(if ordered { "</ol>" } else { "</ul>" });
    }

    fn render_paragraph(&mut self) {
        let mut paragraph = String::new();

        while self.index < self.lines.len() {
            let line = self.lines[self.index];
            let trimmed = line.trim();

            if trimmed.is_empty()
                || fenced_language(trimmed).is_some()
                || heading(trimmed).is_some()
                || is_blockquote(trimmed)
                || list_item(trimmed).is_some()
                || trimmed.starts_with("$$")
            {
                break;
            }

            if !paragraph.is_empty() {
                paragraph.push(' ');
            }
            paragraph.push_str(trimmed);
            self.index += 1;
        }

        if !paragraph.is_empty() {
            self.html
                .push_str(&format!("<p>{}</p>", render_inline(&paragraph)));
        }
    }
}

fn fenced_language(trimmed: &str) -> Option<&str> {
    trimmed.strip_prefix("```").map(str::trim)
}

fn heading(trimmed: &str) -> Option<(usize, &str)> {
    let count = trimmed.chars().take_while(|ch| *ch == '#').count();
    if (1..=6).contains(&count) && trimmed.chars().nth(count) == Some(' ') {
        Some((count, &trimmed[count + 1..]))
    } else {
        None
    }
}

fn is_blockquote(trimmed: &str) -> bool {
    trimmed.starts_with('>')
}

fn list_item(trimmed: &str) -> Option<&str> {
    unordered_list_item(trimmed).or_else(|| ordered_list_item(trimmed))
}

fn unordered_list_item(trimmed: &str) -> Option<&str> {
    ["- ", "* ", "+ "]
        .iter()
        .find_map(|prefix| trimmed.strip_prefix(prefix))
}

fn ordered_list_item(trimmed: &str) -> Option<&str> {
    let dot = trimmed.find('.')?;
    if dot == 0 || !trimmed[..dot].chars().all(|ch| ch.is_ascii_digit()) {
        return None;
    }

    trimmed[dot + 1..].strip_prefix(' ').map(str::trim_start)
}

fn task_list_marker(item: &str) -> Option<(bool, &str)> {
    let rest = item.strip_prefix('[')?;
    let (mark, rest) = rest.split_once(']')?;
    let text = rest.strip_prefix(' ')?;

    match mark {
        " " => Some((false, text)),
        "x" | "X" => Some((true, text)),
        _ => None,
    }
}

fn render_inline(input: &str) -> String {
    let mut output = String::new();
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        if chars[i] == '`' {
            if let Some(end) = find_char(&chars, i + 1, '`') {
                output.push_str("<code>");
                output.push_str(&escape_html(&chars_to_string(&chars[i + 1..end])));
                output.push_str("</code>");
                i = end + 1;
                continue;
            }
        }

        if chars[i] == '$' {
            if let Some(end) = find_char(&chars, i + 1, '$') {
                let math = chars_to_string(&chars[i + 1..end]);
                if !math.trim().is_empty() {
                    output.push_str(&latex_to_mathml(math.trim(), MathDisplay::Inline));
                    i = end + 1;
                    continue;
                }
            }
        }

        if i + 1 < chars.len() && chars[i] == '*' && chars[i + 1] == '*' {
            if let Some(end) = find_double(&chars, i + 2, '*') {
                output.push_str("<strong>");
                output.push_str(&render_inline(&chars_to_string(&chars[i + 2..end])));
                output.push_str("</strong>");
                i = end + 2;
                continue;
            }
        }

        if chars[i] == '*' {
            if let Some(end) = find_char(&chars, i + 1, '*') {
                output.push_str("<em>");
                output.push_str(&render_inline(&chars_to_string(&chars[i + 1..end])));
                output.push_str("</em>");
                i = end + 1;
                continue;
            }
        }

        if chars[i] == '[' {
            if let Some(close_label) = find_char(&chars, i + 1, ']') {
                if close_label + 1 < chars.len() && chars[close_label + 1] == '(' {
                    if let Some(close_url) = find_char(&chars, close_label + 2, ')') {
                        let label = render_inline(&chars_to_string(&chars[i + 1..close_label]));
                        let raw_url = chars_to_string(&chars[close_label + 2..close_url]);
                        if let Some(url) = safe_link_href(&raw_url) {
                            output.push_str(&format!("<a href=\"{}\">{}</a>", url, label));
                        } else {
                            output.push_str(&label);
                        }
                        i = close_url + 1;
                        continue;
                    }
                }
            }
        }

        push_escaped_char(&mut output, chars[i]);
        i += 1;
    }

    output
}

fn find_char(chars: &[char], start: usize, needle: char) -> Option<usize> {
    chars
        .iter()
        .enumerate()
        .skip(start)
        .find_map(|(index, ch)| (*ch == needle).then_some(index))
}

fn find_double(chars: &[char], start: usize, needle: char) -> Option<usize> {
    chars
        .windows(2)
        .enumerate()
        .skip(start)
        .find_map(|(index, pair)| (pair[0] == needle && pair[1] == needle).then_some(index))
}

fn chars_to_string(chars: &[char]) -> String {
    chars.iter().collect()
}

fn safe_link_href(raw: &str) -> Option<String> {
    let href = raw.trim();
    if href.is_empty() {
        return None;
    }

    if href.starts_with('#')
        || href.starts_with('/')
        || href.starts_with("./")
        || href.starts_with("../")
    {
        return Some(escape_attr(href));
    }

    if let Some(colon) = href.find(':') {
        let delimiter = href
            .find(['/', '?', '#'])
            .unwrap_or(href.len());
        if colon < delimiter && is_uri_scheme(&href[..colon]) {
            let scheme = href[..colon].to_ascii_lowercase();
            return matches!(scheme.as_str(), "http" | "https" | "mailto")
                .then(|| escape_attr(href));
        }
    }

    Some(escape_attr(href))
}

fn is_uri_scheme(input: &str) -> bool {
    let mut chars = input.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    first.is_ascii_alphabetic()
        && chars.all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '+' | '-' | '.'))
}

#[derive(Clone, Copy)]
enum MathDisplay {
    Inline,
    Block,
}

fn latex_to_mathml(input: &str, display: MathDisplay) -> String {
    let mut parser = LatexParser::new(input);
    let body = parser.parse_sequence(None);
    let display_attr = match display {
        MathDisplay::Inline => "",
        MathDisplay::Block => " display=\"block\"",
    };
    format!(
        "<math xmlns=\"http://www.w3.org/1998/Math/MathML\"{}>{}</math>",
        display_attr, body
    )
}

struct LatexParser<'a> {
    chars: Vec<char>,
    index: usize,
    source: &'a str,
}

impl<'a> LatexParser<'a> {
    fn new(source: &'a str) -> Self {
        Self {
            chars: source.chars().collect(),
            index: 0,
            source,
        }
    }

    fn parse_sequence(&mut self, until: Option<char>) -> String {
        let mut output = String::new();

        while self.index < self.chars.len() {
            if until.is_some_and(|end| self.chars[self.index] == end) {
                self.index += 1;
                break;
            }

            let mut atom = self.parse_atom();
            loop {
                if self.index >= self.chars.len() {
                    break;
                }

                let op = self.chars[self.index];
                if op != '^' && op != '_' {
                    break;
                }

                self.index += 1;
                let script = self.parse_script();
                atom = if op == '^' {
                    format!("<msup>{}{}</msup>", atom, script)
                } else {
                    format!("<msub>{}{}</msub>", atom, script)
                };
            }

            output.push_str(&atom);
        }

        if output.is_empty() {
            format!("<mtext>{}</mtext>", escape_html(self.source))
        } else {
            output
        }
    }

    fn parse_atom(&mut self) -> String {
        self.skip_whitespace();
        if self.index >= self.chars.len() {
            return String::new();
        }

        match self.chars[self.index] {
            '\\' => self.parse_command(),
            '{' => {
                self.index += 1;
                format!("<mrow>{}</mrow>", self.parse_sequence(Some('}')))
            }
            '+' | '-' | '=' | '*' | '/' | '<' | '>' | '(' | ')' | '[' | ']' | '|' | ',' | ':' => {
                let ch = self.chars[self.index];
                self.index += 1;
                format!("<mo>{}</mo>", escape_html_char(ch))
            }
            ch if ch.is_ascii_digit() => self.parse_number(),
            ch if ch.is_ascii_alphabetic() => self.parse_identifier(),
            ch => {
                self.index += 1;
                format!("<mi>{}</mi>", escape_html_char(ch))
            }
        }
    }

    fn parse_command(&mut self) -> String {
        self.index += 1;
        let start = self.index;
        while self.index < self.chars.len() && self.chars[self.index].is_ascii_alphabetic() {
            self.index += 1;
        }

        let command = chars_to_string(&self.chars[start..self.index]);
        match command.as_str() {
            "frac" => {
                let numerator = self.parse_group_or_atom();
                let denominator = self.parse_group_or_atom();
                format!("<mfrac>{}{}</mfrac>", numerator, denominator)
            }
            "sqrt" => {
                let content = self.parse_group_or_atom();
                format!("<msqrt>{}</msqrt>", content)
            }
            "sum" => "<mo>&#8721;</mo>".to_string(),
            "int" => "<mo>&#8747;</mo>".to_string(),
            "cdot" | "times" => "<mo>&#215;</mo>".to_string(),
            "le" => "<mo>&#8804;</mo>".to_string(),
            "ge" => "<mo>&#8805;</mo>".to_string(),
            "neq" => "<mo>&#8800;</mo>".to_string(),
            "infty" => "<mi>&#8734;</mi>".to_string(),
            "alpha" => "<mi>&#945;</mi>".to_string(),
            "beta" => "<mi>&#946;</mi>".to_string(),
            "gamma" => "<mi>&#947;</mi>".to_string(),
            "delta" => "<mi>&#948;</mi>".to_string(),
            "epsilon" => "<mi>&#949;</mi>".to_string(),
            "lambda" => "<mi>&#955;</mi>".to_string(),
            "mu" => "<mi>&#956;</mi>".to_string(),
            "pi" => "<mi>&#960;</mi>".to_string(),
            "sigma" => "<mi>&#963;</mi>".to_string(),
            "theta" => "<mi>&#952;</mi>".to_string(),
            "omega" => "<mi>&#969;</mi>".to_string(),
            "Delta" => "<mi>&#916;</mi>".to_string(),
            "Omega" => "<mi>&#937;</mi>".to_string(),
            "" => {
                if self.index < self.chars.len() {
                    let ch = self.chars[self.index];
                    self.index += 1;
                    format!("<mo>{}</mo>", escape_html_char(ch))
                } else {
                    String::new()
                }
            }
            _ => format!("<mi>{}</mi>", escape_html(&command)),
        }
    }

    fn parse_group_or_atom(&mut self) -> String {
        self.skip_whitespace();
        if self.index < self.chars.len() && self.chars[self.index] == '{' {
            self.index += 1;
            format!("<mrow>{}</mrow>", self.parse_sequence(Some('}')))
        } else {
            self.parse_atom()
        }
    }

    fn parse_script(&mut self) -> String {
        self.parse_group_or_atom()
    }

    fn parse_number(&mut self) -> String {
        let start = self.index;
        while self.index < self.chars.len()
            && (self.chars[self.index].is_ascii_digit() || self.chars[self.index] == '.')
        {
            self.index += 1;
        }
        format!(
            "<mn>{}</mn>",
            escape_html(&chars_to_string(&self.chars[start..self.index]))
        )
    }

    fn parse_identifier(&mut self) -> String {
        let start = self.index;
        while self.index < self.chars.len() && self.chars[self.index].is_ascii_alphabetic() {
            self.index += 1;
        }
        let ident = chars_to_string(&self.chars[start..self.index]);
        if ident.len() == 1 {
            format!("<mi>{}</mi>", escape_html(&ident))
        } else {
            format!("<mtext>{}</mtext>", escape_html(&ident))
        }
    }

    fn skip_whitespace(&mut self) {
        while self.index < self.chars.len() && self.chars[self.index].is_whitespace() {
            self.index += 1;
        }
    }
}

fn normalize_language(lang: &str) -> &str {
    match lang.trim().to_ascii_lowercase().as_str() {
        "rs" | "rust" => "rust",
        "ts" | "tsx" | "typescript" => "typescript",
        "js" | "jsx" | "javascript" => "javascript",
        _ => "",
    }
}

fn highlight_code(code: &str, lang: &str) -> String {
    let keywords = match lang {
        "rust" => RUST_KEYWORDS,
        "typescript" | "javascript" => JS_KEYWORDS,
        _ => &[][..],
    };

    let mut output = String::new();

    for line in code.lines() {
        output.push_str(&highlight_line(line, keywords, lang));
        output.push('\n');
    }

    output
}

fn highlight_line(line: &str, keywords: &[&str], lang: &str) -> String {
    let chars: Vec<char> = line.chars().collect();
    let mut output = String::new();
    let mut index = 0;

    while index < chars.len() {
        if chars[index] == '/' && index + 1 < chars.len() && chars[index + 1] == '/' {
            output.push_str("<span class=\"tok-comment\">");
            output.push_str(&escape_html(&chars_to_string(&chars[index..])));
            output.push_str("</span>");
            break;
        }

        if chars[index] == '"' || chars[index] == '\'' || chars[index] == '`' {
            let quote = chars[index];
            let start = index;
            index += 1;
            let mut escaped = false;
            while index < chars.len() {
                let ch = chars[index];
                if ch == quote && !escaped {
                    index += 1;
                    break;
                }
                escaped = ch == '\\' && !escaped;
                if ch != '\\' {
                    escaped = false;
                }
                index += 1;
            }
            output.push_str("<span class=\"tok-string\">");
            output.push_str(&escape_html(&chars_to_string(&chars[start..index])));
            output.push_str("</span>");
            continue;
        }

        if chars[index].is_ascii_digit() {
            let start = index;
            index += 1;
            while index < chars.len()
                && (chars[index].is_ascii_alphanumeric()
                    || chars[index] == '_'
                    || chars[index] == '.')
            {
                index += 1;
            }
            output.push_str("<span class=\"tok-number\">");
            output.push_str(&escape_html(&chars_to_string(&chars[start..index])));
            output.push_str("</span>");
            continue;
        }

        if is_ident_start(chars[index]) {
            let start = index;
            index += 1;
            while index < chars.len() && is_ident_continue(chars[index]) {
                index += 1;
            }
            let ident = chars_to_string(&chars[start..index]);
            if keywords.contains(&ident.as_str()) {
                output.push_str("<span class=\"tok-keyword\">");
                output.push_str(&escape_html(&ident));
                output.push_str("</span>");
            } else if lang == "rust" && ident == "self" {
                output.push_str("<span class=\"tok-self\">self</span>");
            } else {
                output.push_str(&escape_html(&ident));
            }
            continue;
        }

        push_escaped_char(&mut output, chars[index]);
        index += 1;
    }

    output
}

fn is_ident_start(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphabetic()
}

fn is_ident_continue(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphanumeric()
}

const RUST_KEYWORDS: &[&str] = &[
    "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum", "extern",
    "false", "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub",
    "ref", "return", "self", "Self", "static", "struct", "super", "trait", "true", "type",
    "unsafe", "use", "where", "while",
];

const JS_KEYWORDS: &[&str] = &[
    "as",
    "async",
    "await",
    "break",
    "case",
    "catch",
    "class",
    "const",
    "continue",
    "default",
    "delete",
    "do",
    "else",
    "export",
    "extends",
    "false",
    "finally",
    "for",
    "from",
    "function",
    "if",
    "import",
    "in",
    "instanceof",
    "interface",
    "let",
    "new",
    "null",
    "of",
    "return",
    "static",
    "super",
    "switch",
    "this",
    "throw",
    "true",
    "try",
    "type",
    "typeof",
    "undefined",
    "var",
    "void",
    "while",
];

fn escape_html(input: &str) -> String {
    let mut output = String::new();
    for ch in input.chars() {
        push_escaped_char(&mut output, ch);
    }
    output
}

fn escape_attr(input: &str) -> String {
    escape_html(input).replace('"', "&quot;")
}

fn push_escaped_char(output: &mut String, ch: char) {
    match ch {
        '&' => output.push_str("&amp;"),
        '<' => output.push_str("&lt;"),
        '>' => output.push_str("&gt;"),
        '"' => output.push_str("&quot;"),
        '\'' => output.push_str("&#39;"),
        _ => output.push(ch),
    }
}

fn escape_html_char(ch: char) -> String {
    let mut output = String::new();
    push_escaped_char(&mut output, ch);
    output
}

#[cfg(test)]
mod tests {
    use super::render_markdown;

    #[test]
    fn renders_headings_tasks_math_and_code() {
        let html = render_markdown(
            "# Title\n\n- [x] Done\n\nEuler $x^2$.\n\n```rust\nlet value = 42; // ok\n```\n",
        );

        assert!(html.contains("<h1>Title</h1>"));
        assert!(html.contains("type=\"checkbox\" disabled checked"));
        assert!(html.contains("<msup><mi>x</mi><mn>2</mn></msup>"));
        assert!(html.contains("<span class=\"tok-keyword\">let</span>"));
        assert!(html.contains("<span class=\"tok-comment\">// ok</span>"));
    }

    #[test]
    fn renders_fraction_to_mathml() {
        let html = render_markdown("$$\n\\frac{a}{b}\n$$");
        assert!(html.contains("display=\"block\""));
        assert!(html.contains("<mfrac><mrow><mi>a</mi></mrow><mrow><mi>b</mi></mrow></mfrac>"));
    }

    #[test]
    fn escapes_untrusted_markup() {
        let html = render_markdown("<script>alert(1)</script>");
        assert!(html.contains("&lt;script&gt;alert(1)&lt;/script&gt;"));
    }

    #[test]
    fn strips_unsafe_link_schemes_but_keeps_safe_links() {
        let html = render_markdown(
            "[bad](javascript:alert(1)) [web](https://example.test/?q=<x>) [rel](docs/readme.md)",
        );

        assert!(!html.contains("javascript:"));
        assert!(html.contains("bad"));
        assert!(html.contains("<a href=\"https://example.test/?q=&lt;x&gt;\">web</a>"));
        assert!(html.contains("<a href=\"docs/readme.md\">rel</a>"));
    }

    #[test]
    fn treats_unknown_fence_info_as_plain_code_without_leaking_attributes() {
        let html = render_markdown("```rust title=\"<bad>\"\nlet s = \"<tag>\";\n```\n");

        assert!(html.contains("<pre class=\"code-block language-\"><code>"));
        assert!(!html.contains("title="));
        assert!(html.contains("&lt;tag&gt;"));
    }

    #[test]
    fn renders_malformed_markdown_without_panicking() {
        let html = render_markdown(
            "Paragraph with [broken](javascript:alert(1)\n\n```rust\nfn main() {\n\n$$\n\\frac{a\n",
        );

        assert!(html.contains("<p>Paragraph with broken</p>"));
        assert!(!html.contains("javascript:"));
        assert!(html.contains("<pre class=\"code-block language-rust\"><code>"));
        assert!(!html.contains("<script"));
    }
}
