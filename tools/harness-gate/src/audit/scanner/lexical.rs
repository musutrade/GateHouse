use super::super::{BlockCommentSyntax, CommentSyntax, StringSyntax};
use std::ops::Range;

pub(super) fn source_line_starts(content: &str) -> Vec<usize> {
    std::iter::once(0)
        .chain(
            content
                .match_indices('\n')
                .map(|(newline_offset, _)| newline_offset + 1),
        )
        .collect()
}

pub(super) fn source_line_at<'a>(
    content: &'a str,
    line_starts: &[usize],
    match_start: usize,
) -> (usize, &'a str, usize) {
    let line_index = line_starts
        .partition_point(|line_start| *line_start <= match_start)
        .saturating_sub(1);
    let line_start = line_starts[line_index];
    let line_end = content[line_start..]
        .find('\n')
        .map(|offset| line_start + offset)
        .unwrap_or(content.len());
    (
        line_index + 1,
        &content[line_start..line_end],
        match_start.saturating_sub(line_start),
    )
}

enum LexicalState<'a> {
    Code,
    String(&'a StringSyntax),
    LineComment {
        start: usize,
    },
    BlockComment {
        syntax: &'a BlockCommentSyntax,
        start: usize,
        depth: usize,
    },
}

fn token_at(bytes: &[u8], offset: usize, token: &str) -> bool {
    bytes[offset..].starts_with(token.as_bytes())
}

pub(super) fn comment_ranges(content: &str, syntax: &CommentSyntax) -> Vec<Range<usize>> {
    let bytes = content.as_bytes();
    let mut ranges = Vec::new();
    let mut state = LexicalState::Code;
    let mut offset = 0;

    while offset < bytes.len() {
        match state {
            LexicalState::Code => {
                if let Some(string) = syntax
                    .strings
                    .iter()
                    .filter(|string| token_at(bytes, offset, &string.start))
                    .max_by_key(|string| string.start.len())
                {
                    offset += string.start.len();
                    state = LexicalState::String(string);
                } else if let Some(block) = syntax
                    .block
                    .iter()
                    .filter(|block| token_at(bytes, offset, &block.start))
                    .max_by_key(|block| block.start.len())
                {
                    let start = offset;
                    offset += block.start.len();
                    state = LexicalState::BlockComment {
                        syntax: block,
                        start,
                        depth: 1,
                    };
                } else if let Some(line) = syntax
                    .line
                    .iter()
                    .filter(|line| token_at(bytes, offset, line))
                    .max_by_key(|line| line.len())
                {
                    let start = offset;
                    offset += line.len();
                    state = LexicalState::LineComment { start };
                } else {
                    offset += 1;
                }
            }
            LexicalState::String(string) => {
                if string
                    .escape
                    .as_deref()
                    .is_some_and(|escape| token_at(bytes, offset, escape))
                {
                    offset += string.escape.as_deref().map_or(0, str::len);
                    offset = (offset + 1).min(bytes.len());
                } else if token_at(bytes, offset, &string.end) {
                    offset += string.end.len();
                    state = LexicalState::Code;
                } else {
                    offset += 1;
                }
            }
            LexicalState::LineComment { start } => {
                if bytes[offset] == b'\n' {
                    ranges.push(start..offset);
                    state = LexicalState::Code;
                }
                offset += 1;
            }
            LexicalState::BlockComment {
                syntax: block,
                start,
                mut depth,
            } => {
                if block.nested && token_at(bytes, offset, &block.start) {
                    depth += 1;
                    offset += block.start.len();
                    state = LexicalState::BlockComment {
                        syntax: block,
                        start,
                        depth,
                    };
                } else if token_at(bytes, offset, &block.end) {
                    depth -= 1;
                    offset += block.end.len();
                    if depth == 0 {
                        ranges.push(start..offset);
                        state = LexicalState::Code;
                    } else {
                        state = LexicalState::BlockComment {
                            syntax: block,
                            start,
                            depth,
                        };
                    }
                } else {
                    offset += 1;
                    state = LexicalState::BlockComment {
                        syntax: block,
                        start,
                        depth,
                    };
                }
            }
        }
    }

    match state {
        LexicalState::LineComment { start } | LexicalState::BlockComment { start, .. } => {
            ranges.push(start..bytes.len())
        }
        LexicalState::Code | LexicalState::String(_) => {}
    }
    ranges
}

pub(super) fn is_comment_offset(ranges: &[Range<usize>], offset: usize) -> bool {
    let index = ranges.partition_point(|range| range.start <= offset);
    index > 0 && ranges[index - 1].contains(&offset)
}
