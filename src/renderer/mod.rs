use std::io::{self, Write};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputMode {
    Text,
    Json,
}

pub struct Renderer<W> {
    writer: W,
    mode: OutputMode,
}

impl<W> Renderer<W>
where
    W: Write,
{
    pub fn new(writer: W, mode: OutputMode) -> Self {
        Self { writer, mode }
    }

    pub fn emit(&mut self, text: &str) -> io::Result<()> {
        if text.is_empty() {
            return Ok(());
        }

        match self.mode {
            OutputMode::Text => self.writer.write_all(text.as_bytes())?,
            OutputMode::Json => writeln!(self.writer, "{{\"data\":\"{}\"}}", json_escape(text))?,
        }

        self.writer.flush()
    }
}

pub fn json_escape(input: &str) -> String {
    let mut output = String::with_capacity(input.len());

    for character in input.chars() {
        match character {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            value if value <= '\u{1F}' => push_control_escape(&mut output, value),
            value => output.push(value),
        }
    }

    output
}

fn push_control_escape(output: &mut String, value: char) {
    let code = value as u32;
    output.push_str("\\u00");
    output.push(hex_digit((code >> 4) & 0xF));
    output.push(hex_digit(code & 0xF));
}

fn hex_digit(value: u32) -> char {
    match value {
        0 => '0',
        1 => '1',
        2 => '2',
        3 => '3',
        4 => '4',
        5 => '5',
        6 => '6',
        7 => '7',
        8 => '8',
        9 => '9',
        10 => 'a',
        11 => 'b',
        12 => 'c',
        13 => 'd',
        14 => 'e',
        _ => 'f',
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emits_plain_text() {
        let mut output = Vec::new();
        let mut renderer = Renderer::new(&mut output, OutputMode::Text);

        renderer.emit("hello").expect("emit should succeed");

        assert_eq!(output, b"hello");
    }

    #[test]
    fn emits_json_lines() {
        let mut output = Vec::new();
        let mut renderer = Renderer::new(&mut output, OutputMode::Json);

        renderer.emit("hello\n").expect("emit should succeed");

        assert_eq!(
            output,
            br#"{"data":"hello\n"}
"#
        );
        assert_eq!(output.last(), Some(&b'\n'));
    }

    #[test]
    fn escapes_json_string_content() {
        assert_eq!(
            json_escape("\"\\\n\r\t\u{0007}"),
            "\\\"\\\\\\n\\r\\t\\u0007"
        );
    }
}
