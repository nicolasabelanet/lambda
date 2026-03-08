use crate::lexer::Span;

pub fn format_span_error(source: &str, message: &str, span: Span) -> String {
    let (line_no, col, line) = line_info(source, span.start);
    let underline_len = span.end.saturating_sub(span.start).max(1);

    let mut output = String::new();
    output.push_str(&format!("error: {message}\n"));
    output.push_str(&format!(" --> line {line_no}, col {col}\n"));
    output.push_str("  |\n");
    output.push_str(&format!("{line_no} | {line}\n"));
    output.push_str("  | ");
    output.push_str(&" ".repeat(col.saturating_sub(1)));
    output.push_str(&"^".repeat(underline_len));
    output
}

fn line_info(source: &str, pos: usize) -> (usize, usize, String) {
    let mut line_start = 0usize;
    let mut line_no = 1usize;

    for line in source.lines() {
        let line_len = line.chars().count();
        let line_end = line_start + line_len;

        if pos <= line_end {
            let col = pos.saturating_sub(line_start) + 1;
            return (line_no, col, line.to_string());
        }

        line_start = line_end + 1;
        line_no += 1;
    }

    let last_line = source.lines().last().unwrap_or("");
    let last_line_len = last_line.chars().count();
    (
        line_no.saturating_sub(1).max(1),
        last_line_len + 1,
        last_line.to_string(),
    )
}

#[cfg(test)]
mod tests {
    use crate::{diagnostic::format_span_error, lexer::Span};

    #[test]
    fn test_format_span_error_end_of_line() {
        let output = format_span_error("\\x", "expected '.'", Span { start: 2, end: 2 });
        let expected = "error: expected '.'\n --> line 1, col 3\n  |\n1 | \\x\n  |   ^";
        assert_eq!(output, expected);
    }

    #[test]
    fn test_format_span_error_range() {
        let output = format_span_error("abc", "expected term", Span { start: 1, end: 3 });
        let expected = "error: expected term\n --> line 1, col 2\n  |\n1 | abc\n  |  ^^";
        assert_eq!(output, expected);
    }
}
