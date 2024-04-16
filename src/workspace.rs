use log::{debug, error, info, warn};
use lsp_types::{
    Diagnostic, DiagnosticSeverity, DocumentChanges, Location, OneOf, Position, Range,
    TextDocumentContentChangeEvent, TextEdit, Url,
};
use std::ffi::OsStr;
use std::fs;
use std::path::Path;
use std::{collections::HashMap, io::Error};
use tree_sitter::{InputEdit, Node, Parser, Point, QueryCursor, Tree};
use tree_sitter_xml;

use self::sync::TextSync;
mod sync;

pub struct IEF_Policy {
    text: TextSync,
    tree: Tree,
    pub id: String,
    pub base_id: Option<String>,
}

fn base_policy_query(text: &str, root_node: Node) -> Option<String> {
    let query_str = "(element 
      (STag 
        (Name) @tagName) 
      (content 
        (element
          (STag 
            (Name) @innerName) 
          (content) @basePolicyId
          (#eq? @innerName \"PolicyId\")) @content 
        (#eq? @tagName \"BasePolicy\")))
      ";
    let query = tree_sitter::Query::new(&tree_sitter_xml::language_xml(), query_str).unwrap();
    let mut cursor = QueryCursor::new();
    return cursor
        .matches(&query, root_node, text.as_bytes())
        .filter_map(|m| m.captures.last())
        .filter_map(|c| c.node.utf8_text(text.as_bytes()).ok())
        .map(|s| String::from(s).replace("\"", ""))
        .last();
}

fn base_policy_query_range(text: &str, root_node: Node) -> Option<Range> {
    let query_str = "(element 
      (STag 
        (Name) @tagName) 
      (content 
        (element
          (STag 
            (Name) @innerName) 
          (content) @basePolicyId
          (#eq? @innerName \"PolicyId\")) @content 
        (#eq? @tagName \"BasePolicy\")))
      ";
    let query = tree_sitter::Query::new(&tree_sitter_xml::language_xml(), query_str).unwrap();
    let mut cursor = QueryCursor::new();
    return cursor
        .matches(&query, root_node, text.as_bytes())
        .filter_map(|m| m.captures.last())
        .map(|c| {
            let node = c.node;
            let start = Position {
                line: node.start_position().row as u32,
                character: node.start_position().column as u32,
            };
            let end = Position {
                line: node.start_position().row as u32,
                character: node.start_position().column as u32,
            };
            return Range { start, end };
        })
        .last();
}

fn id_query(text: &str, root_node: Node) -> Option<String> {
    let query = tree_sitter::Query::new(
        &tree_sitter_xml::language_xml(),
        "(element (STag (Name) (Attribute (Name) @name (AttValue) @policyId (#eq? @name \"PolicyId\")))) ",
    )
    .unwrap();
    let mut cursor = QueryCursor::new();
    return cursor
        .matches(&query, root_node, text.as_bytes())
        .filter_map(|m| m.captures.last())
        .filter_map(|c| c.node.utf8_text(text.as_bytes()).ok())
        .map(|s| String::from(s).replace("\"", ""))
        .last();
}

impl IEF_Policy {
    fn new(sitter: &mut Parser, path: &String) -> Option<Self> {
        let path = path.clone();
        let text = match fs::read_to_string(&path).ok() {
            Some(text) => TextSync::new(text),
            None => return None,
        };
        let tree = match sitter.parse(text.text(), None) {
            None => return None,
            Some(tree) => tree,
        };
        let id = id_query(text.text(), tree.root_node()).unwrap_or(String::new());
        let base_id = base_policy_query(text.text(), tree.root_node());
        return Some(IEF_Policy {
            tree,
            text,
            id,
            base_id,
        });
    }
}
struct UpdateDocError {
    msg: String,
}
impl UpdateDocError {
    fn new(msg: &str) -> Self {
        return Self {
            msg: String::from(msg),
        };
    }
}
pub struct IEF_Workspace<'a> {
    root_path: &'a str,
    //appsettings: Option<Tree>,
    //app_settings_path: Option<Path>,
    policies: HashMap<String, IEF_Policy>,
    parser: Parser,
}
impl IEF_Workspace<'_> {
    pub fn find_policy_by_id(&self, id: &str) -> Option<&IEF_Policy> {
        self.policies.values().find(|p| p.id.as_str() == id)
    }

    fn handle_edit(&mut self, uri: Url, edit: &TextEdit) -> Result<(), UpdateDocError> {
        let policy = match self
            .policies
            .get_mut(uri.to_file_path().unwrap().to_str().unwrap())
        {
            Some(p) => p,
            None => return Err(UpdateDocError::new("Document not found")),
        };
        let start_line = edit.range.start.line.try_into().unwrap();
        let start_char = edit.range.start.character.try_into().unwrap();
        let end_line = edit.range.end.line.try_into().unwrap();
        let end_char = edit.range.end.character.try_into().unwrap();
        if edit.new_text == "" {
            policy.tree.edit(&InputEdit {
                start_byte: policy.text.byte_pos(start_line, start_char),
                start_position: Point::new(start_line, start_char),
                old_end_byte: policy.text.byte_pos(end_line, end_char),
                old_end_position: Point::new(end_line, end_char),
                new_end_byte: policy.text.byte_pos(start_line, start_char),
                new_end_position: Point::new(start_line, start_char),
            });
        } else {
            let new_lines = edit.new_text.lines().count();
            let mut new_chars = edit.new_text.lines().last().unwrap().len();
            if new_lines == 0 {
                new_chars += start_char;
            }
            let new_bytes = edit.new_text.len();
            policy.tree.edit(&InputEdit {
                start_byte: policy.text.byte_pos(start_line, start_char),
                start_position: Point::new(start_line, start_char),
                old_end_byte: policy.text.byte_pos(end_line, end_char),
                old_end_position: Point::new(end_line, end_char),
                new_end_byte: policy.text.byte_pos(start_line + new_lines, new_chars) + new_bytes,
                new_end_position: Point::new(start_line + new_lines, new_chars),
            });
        }
        policy.text.edit(edit);
        policy.tree = self
            .parser
            .parse(policy.text.text(), Some(&policy.tree))
            .unwrap_or(policy.tree.clone());
        Ok(())
    }
    pub fn update_document(
        &mut self,
        document: Url,
        changes: Vec<TextDocumentContentChangeEvent>,
    ) -> Result<(), Error> {
        info!("{changes:?}");
        let edits: Vec<TextEdit> = changes
            .iter()
            .filter_map(|change| match change.range {
                Some(range) => Some(TextEdit {
                    range: range,
                    new_text: change.text.clone(),
                }),
                None => None,
            })
            .collect();
        for edit in edits {
            self.handle_edit(document.clone(), &edit);
        }
        Ok(())
    }
    pub fn get_diagnostics(&self) -> HashMap<String, Vec<Diagnostic>> {
        self.policies
            .iter()
            .filter_map(|(path, policy)| match &policy.base_id {
                None => None,
                Some(base_id) => match self.find_policy_by_id(base_id.as_str()) {
                    Some(p) => None,
                    None => {
                        let range_opt =
                            base_policy_query_range(&policy.text.text(), policy.tree.root_node());
                        if range_opt.is_none() {
                            return None;
                        }
                        return Some((
                            to_uri(path),
                            vec![Diagnostic {
                                range: range_opt.unwrap(),
                                severity: Some(DiagnosticSeverity::ERROR),
                                code: None,
                                code_description: None,
                                source: Some(String::from("IEF_LSP")),
                                related_information: None,
                                tags: None,
                                data: None,
                                message: format!("Policy with ID {} does not exist!", base_id),
                            }],
                        ));
                    }
                },
            })
            .collect()
    }
}

pub fn find_ief_files(path: &str) -> Vec<String> {
    let mut path = path;
    if path.starts_with("file://") {
        path = &path[7..];
    }
    let path = Path::new(OsStr::new(path));
    info!("{:?}", path);
    if !path.exists() {
        error!("Find IEF Files path does not exist! {:?}", path);
        return vec![];
    }
    let metadata = match fs::metadata(path) {
        Err(e) => return vec![],
        Ok(meta) => meta,
    };
    let is_dir = metadata.file_type().is_dir();
    let mut dir_path: &Path;
    if is_dir {
        dir_path = path;
    } else {
        dir_path = match path.parent() {
            Some(p) => p,
            None => {
                error!("Non-directory with no parent! I'm not sure how this is possible");
                return vec![];
            }
        }
    }
    info!(
        "Dir path is {} ",
        dir_path.to_str().unwrap_or("no path str")
    );
    //Just in case
    if !dir_path.exists() {
        error!("Find IEF Files path does not exist! {:?}", path);
        return vec![];
    }
    return match dir_path.read_dir() {
        Err(e) => {
            error!("Error reading directory {:?} \n {:?}", dir_path, e);
            return vec![];
        }
        Ok(dir_res) => dir_res
            .filter_map(|dir_entry_res| dir_entry_res.ok())
            .map(|dir_entry| dir_entry.path())
            .filter(|path_buf| path_buf.extension() == Some(&OsStr::new("xml")))
            .filter_map(|path_buf| match path_buf.to_str() {
                None => None,
                Some(s) => Some(String::from(s)),
            })
            .collect(),
    };
}

//FYI Url has thesse fns built in and I am a dummy
//For now, we will just assume a local file system
//
fn to_uri(path: &str) -> String {
    return format!("file://{}", path);
}
//Remove file prefix if it exists
fn from_uri(path: &str) -> String {
    let mut p = String::from(path);
    p.replace("file://", "")
}
//fn parse_app_settings(path: Option<String>) -> Option<String> {}
pub fn new_workspace<'a>(root_path: &'a str) -> IEF_Workspace<'a> {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_xml::language_xml())
        .unwrap();
    let policy_paths = find_ief_files(root_path);
    let policies = HashMap::from_iter(policy_paths.iter().filter_map(|p| {
        match IEF_Policy::new(&mut parser, p) {
            None => None,
            Some(pol) => Some((String::from(p), pol)),
        }
    }));
    return IEF_Workspace {
        root_path,
        policies,
        parser,
    };
}
