use logos::Logos;
use muse_types::{Diagnostic, Span};
use std::fmt;

#[derive(Logos, Debug, Clone, PartialEq, Eq)]
pub enum TokenKind {
    #[token("namespace")]
    Namespace,
    #[token("using")]
    Using,
    #[token("as")]
    As,
    #[token("asset")]
    Asset,
    #[token("use")]
    Use,
    #[token("skill")]
    Skill,
    #[token("intent")]
    Intent,
    #[token("timeline")]
    Timeline,
    #[token("tempo")]
    Tempo,
    #[token("music")]
    Music,
    #[token("audio")]
    Audio,
    #[token("lyric")]
    Lyric,
    #[token("visual")]
    Visual,
    #[token("orchestrate")]
    Orchestrate,
    #[token("directing")]
    Directing,
    #[token("instance")]
    Instance,
    #[token("for")]
    For,
    #[token("{")]
    LBrace,
    #[token("}")]
    RBrace,
    #[token("[")]
    LBracket,
    #[token("]")]
    RBracket,
    #[token("(")]
    LParen,
    #[token(")")]
    RParen,
    #[token(";")]
    Semicolon,
    #[token(":")]
    Colon,
    #[token(",")]
    Comma,
    #[token("=")]
    Equals,
    #[token("/")]
    Slash,
    #[token("..")]
    Range,
    #[regex(r#"\"([^\"\\]|\\.)*\""#)]
    String,
    #[regex(r"-?(?:0|[1-9][0-9]*)(?:\.[0-9]+)?")]
    Number,
    #[regex(r"[[:alpha:]_][[:alnum:]_./-]*")]
    Identifier,
    #[regex(r"#[^\r\n]*", logos::skip, allow_greedy = true)]
    Comment,
    #[regex(r"[ \t\r\n\f]+", logos::skip)]
    Whitespace,
}

impl fmt::Display for TokenKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Namespace => "namespace",
            Self::Using => "using",
            Self::As => "as",
            Self::Asset => "asset",
            Self::Use => "use",
            Self::Skill => "skill",
            Self::Intent => "intent",
            Self::Timeline => "timeline",
            Self::Tempo => "tempo",
            Self::Music => "music",
            Self::Audio => "audio",
            Self::Lyric => "lyric",
            Self::Visual => "visual",
            Self::Orchestrate => "orchestrate",
            Self::Directing => "directing",
            Self::Instance => "instance",
            Self::For => "for",
            Self::LBrace => "{",
            Self::RBrace => "}",
            Self::LBracket => "[",
            Self::RBracket => "]",
            Self::LParen => "(",
            Self::RParen => ")",
            Self::Semicolon => ";",
            Self::Colon => ":",
            Self::Comma => ",",
            Self::Equals => "=",
            Self::Slash => "/",
            Self::Range => "..",
            Self::String => "string",
            Self::Number => "number",
            Self::Identifier => "identifier",
            Self::Comment => "comment",
            Self::Whitespace => "whitespace",
        })
    }
}

#[derive(Debug, Clone)]
pub struct Token {
    pub kind: TokenKind,
    pub lexeme: String,
    pub start: usize,
    pub end: usize,
}

impl Token {
    pub fn span(&self, source: &str) -> Span {
        span(source, self.start, self.end)
    }
}

pub fn lex(source: &str) -> (Vec<Token>, Vec<Diagnostic>) {
    let mut lexer = TokenKind::lexer(source);
    let mut tokens = Vec::new();
    let mut diagnostics = Vec::new();
    while let Some(result) = lexer.next() {
        let range = lexer.span();
        match result {
            Ok(kind) => tokens.push(Token {
                kind,
                lexeme: source[range.clone()].to_owned(),
                start: range.start,
                end: range.end,
            }),
            Err(()) => {
                let raw = &source[range.clone()];
                let (message, help) = if raw == "/" && source[range.start..].starts_with("//") {
                    ("`//` is not a Muse comment", "use `#` for a single-line comment")
                } else {
                    (
                        "unrecognised character",
                        "remove this character or place it inside a string",
                    )
                };
                diagnostics.push(Diagnostic::error(
                    format!("{message}: `{raw}`; {help}"),
                    span(source, range.start, range.end),
                ));
            }
        }
    }
    (tokens, diagnostics)
}

pub fn span(source: &str, start: usize, end: usize) -> Span {
    let prefix = &source[..start];
    let line = prefix.bytes().filter(|byte| *byte == b'\n').count() + 1;
    let column = prefix.rfind('\n').map_or(start + 1, |newline| start - newline);
    Span::new(start, end, line, column)
}
