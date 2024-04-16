use lsp_types::{Position, Range, TextEdit};
pub struct TextSync {
    raw_text: String,
}

fn delete_range(txt: &mut str, start_byte: usize, end_byte: usize) -> String {
    let (pre_delete, extra) = txt.split_at_mut(start_byte);
    let (_, post_delete) = extra.split_at_mut(end_byte - start_byte);
    let mut new_str = String::from(pre_delete);
    new_str.push_str(post_delete);
    return new_str;
}

fn insert_text(txt: &mut str, range: &str, byte: usize) -> String {
    let (pre_insert, post_insert) = txt.split_at_mut(byte);
    let mut new_str = String::from(pre_insert);
    new_str.push_str(range);
    new_str.push_str(post_insert);
    return new_str;
}

impl TextSync {
    pub fn new(text: String) -> Self {
        TextSync { raw_text: text }
    }

    pub fn text(&self) -> &str {
        self.raw_text.as_str()
    }

    pub fn edit(&mut self, edit: &TextEdit) {
        let start = edit.range.start;
        let end = edit.range.end;
        let start_byte = self.byte_pos(
            start.line.try_into().unwrap(),
            start.character.try_into().unwrap(),
        );
        let end_byte = self.byte_pos(
            end.line.try_into().unwrap(),
            end.character.try_into().unwrap(),
        );
        let text = edit.new_text.clone();
        let new_text = match text.as_str() {
            "" => delete_range(&mut self.raw_text, start_byte, end_byte),
            _ => insert_text(&mut self.raw_text, text.as_str(), start_byte),
        };
        self.raw_text = new_text;
    }
    pub fn lines(&self) -> usize {
        self.raw_text.lines().count()
    }

    pub fn characters(&self, line: usize) -> usize {
        match self.raw_text.lines().nth(line) {
            Some(l) => l.len(),
            None => 0,
        }
    }

    pub fn byte_pos(&self, line: usize, character: usize) -> usize {
        let mut byte_count = 0;
        for (i, l) in self.raw_text.lines().enumerate() {
            if i == line {
                byte_count += character;
                break;
            }
            byte_count += l.len();
            byte_count += 1; // for the \n
        }
        return byte_count;
    }
}
#[cfg(test)]
mod test {
    use lsp_types::TextEdit;

    use super::TextSync;

    #[test]
    fn test_tests() {
        assert_eq!(2 + 2, 4);
    }
    #[test]
    fn test_lines() {
        let obj = TextSync {
            raw_text: String::from("\n\n\n"),
        };
        assert_eq!(obj.lines(), 3)
    }
    #[test]
    fn test_characters() {
        let obj = TextSync {
            raw_text: String::from("\n\n\n"),
        };
        assert_eq!(obj.characters(1), 0)
    }
    #[test]
    fn test_byte_pos() {
        let obj = TextSync {
            raw_text: String::from("\n\n\n"),
        };
        assert_eq!(obj.byte_pos(1, 0), 1);
        assert_eq!(obj.byte_pos(1, 1), 2);
    }
    #[test]
    fn test_byte_pos_with_text() {
        let obj = TextSync {
            raw_text: String::from("abc\n\n\n"),
        };
        assert_eq!(obj.byte_pos(0, 2), 2);
        assert_eq!(obj.byte_pos(1, 0), 4);
    }

    #[test]
    fn test_delete_text() {
        let mut obj = TextSync {
            raw_text: String::from("abc\nabc\n\n"),
        };
        let s = TextEdit {
            range: lsp_types::Range {
                start: lsp_types::Position {
                    line: 1,
                    character: 0,
                },
                end: lsp_types::Position {
                    line: 1,
                    character: 2,
                },
            },
            new_text: String::from(""),
        };
        obj.edit(&s);
        assert_eq!(obj.raw_text.as_str(), "abc\nc\n\n")
    }
    #[test]
    fn test_insert_text() {
        let mut obj = TextSync {
            raw_text: String::from("abc\nc\n\n"),
        };
        let s = TextEdit {
            range: lsp_types::Range {
                start: lsp_types::Position {
                    line: 1,
                    character: 0,
                },
                end: lsp_types::Position {
                    line: 1,
                    character: 0,
                },
            },
            new_text: String::from("ab"),
        };
        obj.edit(&s);
        assert_eq!(obj.raw_text.as_str(), "abc\nabc\n\n")
    }
}
