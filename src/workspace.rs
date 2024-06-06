use log::{debug, error, info, warn};
use lsp_types::{
    Diagnostic, DiagnosticSeverity, DocumentChanges, Location, OneOf,
    TextDocumentContentChangeEvent, TextEdit, Url,
};
use std::ffi::OsStr;
use std::fs;
use std::path::Path;
use std::{collections::HashMap, io::Error};
use tree_sitter::{InputEdit, Node, Parser, Point, Query, QueryCursor, Tree};
use tree_sitter_xml;

use self::queries::{base_policy_query, id_query, null_range, IEFQuery, IEFQueryMatch};
use self::sync::TextSync;
mod queries;
mod sync;

pub struct IEF_Policy {
    text: TextSync,
    tree: Tree,
    pub id: String,
    pub base_id: Option<IEFQueryMatch>,
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
        let mut new_policy = IEF_Policy {
            tree,
            text,
            id: String::from(""),
            base_id: None,
        };
        new_policy.compute_ids();
        return Some(new_policy);
    }

    pub fn handle_edit(
        &mut self,
        parser: &mut Parser,
        edit: &TextEdit,
    ) -> Result<(), UpdateDocError> {
        let start_line = edit.range.start.line.try_into().unwrap();
        let start_char = edit.range.start.character.try_into().unwrap();
        let end_line = edit.range.end.line.try_into().unwrap();
        let end_char = edit.range.end.character.try_into().unwrap();
        if edit.new_text == "" {
            self.tree.edit(&InputEdit {
                start_byte: self.text.byte_pos(start_line, start_char),
                start_position: Point::new(start_line, start_char),
                old_end_byte: self.text.byte_pos(end_line, end_char),
                old_end_position: Point::new(end_line, end_char),
                new_end_byte: self.text.byte_pos(start_line, start_char),
                new_end_position: Point::new(start_line, start_char),
            });
        } else {
            let new_lines = edit.new_text.lines().count();
            let mut new_chars = edit.new_text.lines().last().unwrap().len();
            if new_lines == 0 {
                new_chars += start_char;
            }
            let new_bytes = edit.new_text.len();
            self.tree.edit(&InputEdit {
                start_byte: self.text.byte_pos(start_line, start_char),
                start_position: Point::new(start_line, start_char),
                old_end_byte: self.text.byte_pos(end_line, end_char),
                old_end_position: Point::new(end_line, end_char),
                new_end_byte: self.text.byte_pos(start_line + new_lines, new_chars) + new_bytes,
                new_end_position: Point::new(start_line + new_lines, new_chars),
            });
        }
        self.text.edit(edit);
        self.tree = parser
            .parse(self.text.text(), Some(&self.tree))
            .unwrap_or(self.tree.clone());
        self.compute_ids();
        return Ok(());
    }

    pub fn compute_ids(&mut self) {
        let id_query = id_query();
        let base_query = base_policy_query();
        let id = id_query
            .first(self.tree.root_node(), self.text.text())
            .unwrap_or(queries::IEFQueryMatch {
                txt: String::from(""),
                range: null_range(),
            })
            .txt;
        let base_id = base_query.first(self.tree.root_node(), self.text.text());
        self.id = id;
        self.base_id = base_id;
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
        self.policies
            .values()
            .find(|p| p.id.as_str().to_lowercase() == id.to_lowercase())
    }

    fn handle_edit(&mut self, uri: Url, edit: &TextEdit) -> Result<(), UpdateDocError> {
        let policy = match self
            .policies
            .get_mut(uri.to_file_path().unwrap().to_str().unwrap())
        {
            Some(p) => p,
            None => return Err(UpdateDocError::new("Document not found")),
        };
        policy.handle_edit(&mut self.parser, edit)
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
            .filter_map(|(path, policy)| {
                let mut diagnostics = vec![];
                match &policy.base_id {
                    None => {}
                    Some(base_id) => match self.find_policy_by_id(base_id.txt.as_str()) {
                        Some(p) => {
                            info!("Calculated diagnostics {diagnostics:?} for file {path:?}");
                        }
                        None => {
                            diagnostics.push(Diagnostic {
                                range: base_id.range,
                                severity: Some(DiagnosticSeverity::ERROR),
                                code: None,
                                code_description: None,
                                source: Some(String::from("IEF_LSP")),
                                related_information: None,
                                tags: None,
                                data: None,
                                message: format!(
                                    "Policy with ID {:?} does not exist!",
                                    base_id.txt
                                ),
                            });
                            info!("Calculated diagnostics {diagnostics:?} for file {path:?}");
                        }
                    },
                }
                info!("Policy id {:?}", policy.id);
                if policy.id.as_str() == "" {
                    diagnostics.push(Diagnostic {
                        //TODO search for TrustFramework base tag
                        range: null_range(),
                        severity: Some(DiagnosticSeverity::ERROR),
                        code: None,
                        code_description: None,
                        source: Some(String::from("IEF_LSP")),
                        related_information: None,
                        tags: None,
                        data: None,
                        //TODO liven this message up
                        message: format!("Policy requires a Policy ID"),
                    });
                }
                return Some((to_uri(path), diagnostics));
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
