use log::{error, info};
use lsp_types::{Position, Range};
use tree_sitter::{InputEdit, Node, Parser, Point, Query, QueryCursor, Tree};
pub struct IEFQuery {
    query: Query,
}
pub struct IEFQueryMatch {
    txt: String,
    range: Range,
}

fn get_range(node: &Node) -> Range {
    let start = Position {
        line: node.start_position().row as u32,
        character: node.start_position().column as u32,
    };
    let end = Position {
        line: node.start_position().row as u32,
        character: node.start_position().column as u32,
    };
    return Range { start, end };
}
impl IEFQuery {
    pub fn new(query_txt: &str, case_sensitive: bool) -> Self {
        let query = Query::new(&tree_sitter_xml::language_xml(), query_txt).unwrap();
        IEFQuery { query }
    }
    pub fn first(&self, root_node: Node, text: &str) -> Option<IEFQueryMatch> {
        let mut cursor = QueryCursor::new();
        return cursor
            .matches(&self.query, root_node, text.as_bytes())
            .filter_map(|m| m.captures.first())
            .filter_map(|c| match c.node.utf8_text(text.as_bytes()) {
                Ok(s) => Some(IEFQueryMatch {
                    range: get_range(&c.node),
                    txt: String::from(s),
                }),
                Err(e) => {
                    error!("Could not find text in query node !");
                    None
                }
            })
            .last();
    }
    pub fn all(&self, root_node: Node, text: &str) -> Vec<IEFQueryMatch> {
        let mut cursor = QueryCursor::new();
        return cursor
            .matches(&self.query, root_node, text.as_bytes())
            .filter_map(|m| m.captures.first())
            .filter_map(|c| match c.node.utf8_text(text.as_bytes()) {
                Ok(s) => Some(IEFQueryMatch {
                    range: get_range(&c.node),
                    txt: String::from(s),
                }),
                Err(e) => {
                    error!("Could not find text in query node !");
                    None
                }
            })
            .collect();
    }
}

pub fn base_policy_query(text: &str, root_node: Node) -> Option<String> {
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

pub fn base_policy_query_range(text: &str, root_node: Node) -> Option<Range> {
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

pub fn id_query(text: &str, root_node: Node) -> Option<String> {
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
