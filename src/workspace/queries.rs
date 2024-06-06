use std::collections::HashMap;

use log::{error, info};
use lsp_types::{Position, Range};
use tree_sitter::{InputEdit, Node, Parser, Point, Query, QueryCursor, QueryMatch, Tree};

pub struct IEFQuery {
    query: Query,
}

pub struct IEFQueryMatch {
    pub txt: String,
    pub range: Range,
}

pub struct IEFDefinitionMatch {
    pub id: String,
    pub tag_name: String,
    pub id_range: Range,
    //IDK if we want a full tag name or content
}
#[derive(Clone)]
pub struct XMLElement {
    pub name: String,
    pub attrs: HashMap<String, String>,
}

pub fn null_range() -> Range {
    let start = Position {
        line: 0,
        character: 0,
    };
    let end = Position {
        line: 0,
        character: 0,
    };
    return Range { start, end };
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

pub fn get_tag_name<'a>(root_node: &'a Node, pos: Position) -> Option<Node<'a>> {
    let location = Point {
        row: pos.line as usize,
        column: pos.character as usize,
    };
    let mut cur_node = root_node.named_descendant_for_point_range(location, location);
    while cur_node.is_some() && cur_node.unwrap().grammar_name() != "element" {
        cur_node = cur_node.unwrap().parent();
        if let Some(n) = cur_node {
            dbg!(n.to_string());
        }
    }
    cur_node
}

//I forget why this abstraction exists
impl IEFQuery {
    pub fn new(query_txt: &str) -> Self {
        let query = Query::new(&tree_sitter_xml::language_xml(), query_txt).unwrap();
        IEFQuery { query }
    }

    pub fn first(&self, root_node: Node, text: &str) -> Option<IEFQueryMatch> {
        let mut cursor = QueryCursor::new();
        return cursor
            .matches(&self.query, root_node, text.as_bytes())
            .filter_map(|m| m.captures.last())
            .filter_map(|c| match c.node.utf8_text(text.as_bytes()) {
                Ok(s) => Some(IEFQueryMatch {
                    range: get_range(&c.node),
                    txt: String::from(s).replace("\"", ""),
                }),
                Err(e) => {
                    error!("Could not find text in query node !");
                    None
                }
            })
            .next();
    }

    fn parse_definition_match(m: QueryMatch, text: &str) -> Option<IEFDefinitionMatch> {
        let first = m.captures.first();
        let last = m.captures.last();
        if first.is_none() || last.is_none() {
            return None;
        }
        let tag_name_capt = first.unwrap();
        let id_capt = last.unwrap();
        let tag_name_res = tag_name_capt.node.utf8_text(text.as_bytes());
        let id_name_res = id_capt.node.utf8_text(text.as_bytes());
        if tag_name_res.is_err() || id_name_res.is_err() {
            return None;
        }
        Some(IEFDefinitionMatch {
            id: String::from(id_name_res.unwrap()).replace("\"", ""),
            //Tag name not in quotes so we don't replace
            tag_name: String::from(tag_name_res.unwrap()),
            id_range: get_range(&id_capt.node),
        })
    }

    pub fn all(&self, root_node: Node, text: &str) -> Vec<IEFDefinitionMatch> {
        let mut cursor = QueryCursor::new();
        return cursor
            .matches(&self.query, root_node, text.as_bytes())
            .filter_map(|m| IEFQuery::parse_definition_match(m, text))
            .collect();
    }
}

pub fn parse_attrs(node: Node, text: &str) -> HashMap<String, String> {
    let query = attr_query();
    let mut cursor = QueryCursor::new();
    return HashMap::from_iter(
        cursor
            .matches(&query.query, node, text.as_bytes())
            .filter_map(|m| {
                let key = m.captures.first();
                let value = m.captures.last();
                if key.is_none() || value.is_none() {
                    return None;
                }
                let key = key.unwrap();
                let value = value.unwrap();
                let key_txt = key.node.utf8_text(text.as_bytes());
                let val_txt = value.node.utf8_text(text.as_bytes());
                if key_txt.is_ok() && val_txt.is_ok() {
                    return Some((
                        String::from(key_txt.unwrap()).replace("\"", ""),
                        String::from(val_txt.unwrap()).replace("\"", ""),
                    ));
                }
                return None;
            }),
    );
}

pub fn parse_tag(node: Node, text: &str) -> Option<XMLElement> {
    dbg!(node.to_string());
    if let Some(name) = tag_name_query().first(node, text) {
        return Some(XMLElement {
            name: name.txt,
            attrs: parse_attrs(node, text),
        });
    } else {
        return None;
    }
}

pub fn base_policy_query() -> IEFQuery {
    IEFQuery::new(
        "(element 
      (STag 
        (Name) @tagName) 
      (content 
        (element
          (STag 
            (Name) @innerName) (content) @basePolicyId
          (#eq? @innerName \"PolicyId\")) @content 
        (#eq? @tagName \"BasePolicy\")))
      ",
    )
}

pub fn id_query() -> IEFQuery {
    IEFQuery::new(
        "(element 
         (STag 
          (Name) 
          (Attribute 
           (Name) @name 
           (AttValue) @PolicyId 
           (#eq? @name \"PolicyId\")
           )
          )
         )",
    )
}

pub fn tag_name_query() -> IEFQuery {
    IEFQuery::new(
        "[
        (STag (Name) @tagName)
        (EmptyElemTag (Name) @tagName)
        ]",
    )
}

pub fn definition_query() -> IEFQuery {
    IEFQuery::new(
        "(element 
         (STag 
          (Name) @tagName
          (Attribute 
           (Name)  @attrName
           (AttValue) @id 
           (#eq? @attrName \"Id\")
           )
          )
         )",
    )
}

pub fn attr_query() -> IEFQuery {
    IEFQuery::new(
        "(
           (Name)  @attrName
           (AttValue) @id 
        )",
    )
}
#[cfg(test)]
mod test {
    use std::any::Any;

    use crate::workspace::queries::{base_policy_query, definition_query, parse_tag};

    use super::{get_tag_name, id_query};
    use log::error;
    use lsp_types::Position;
    use tree_sitter::Tree;

    fn get_test_str() -> (Tree, String) {
        let s = String::from(" 
            <?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>
            <TrustFrameworkPolicy
              xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\"
              xmlns:xsd=\"http://www.w3.org/2001/XMLSchema\"
              xmlns=\"http://schemas.microsoft.com/online/cpim/schemas/2013/06\"
              PolicySchemaVersion=\"0.3.0.0\"
              TenantId=\"yourtenant.onmicrosoft.com\"
              PolicyId=\"B2C_1A_ProfileEdit\"
              PublicPolicyUri=\"http://yourtenant.onmicrosoft.com/B2C_1A_ProfileEdit\">
              <BasePolicy>
                <TenantId>yourtenant.onmicrosoft.com</TenantId>
                <PolicyId>B2C_1A_TrustFrameworkExtensions</PolicyId>
              </BasePolicy>
               
              <RelyingParty>
                <DefaultUserJourney ReferenceId=\"ProfileEdit\"/>
                <TechnicalProfile Id=\"PolicyProfile\">
                  <DisplayName>PolicyProfile</DisplayName>
                  <Protocol Name=\"OpenIdConnect\" />
                  <OutputClaims>
                    <OutputClaim ClaimTypeReferenceId=\"objectId\" PartnerClaimType=\"sub\"/>
                    <OutputClaim ClaimTypeReferenceId=\"tenantId\" AlwaysUseDefaultValue=\"true\" DefaultValue=\"{Policy:TenantObjectId}\" />
                  </OutputClaims>
                  <SubjectNamingInfo ClaimType=\"sub\" />
                </TechnicalProfile>
              </RelyingParty>
            </TrustFrameworkPolicy>
        ");
        let mut t = tree_sitter::Parser::new();
        t.set_language(&tree_sitter_xml::language_xml());
        return (t.parse(s.as_str(), None).unwrap(), s);
    }
    #[test]
    fn test_id_query() {
        let (t, s) = get_test_str();
        let query = id_query();
        let res = query.first(t.root_node(), s.as_str());
        assert!(res.is_some());
        let id = res.unwrap();
        assert_eq!(id.txt, "B2C_1A_ProfileEdit");
    }
    #[test]
    fn test_base_id_query() {
        let (t, s) = get_test_str();
        let query = base_policy_query();
        let res = query.first(t.root_node(), s.as_str());
        assert!(res.is_some());
        let id = res.unwrap();
        assert_eq!(id.txt, "B2C_1A_TrustFrameworkExtensions");
    }

    #[test]
    fn test_def_query() {
        let (t, s) = get_test_str();
        let query = definition_query();
        let res = query.all(t.root_node(), s.as_str());
        assert_eq!(res.len(), 1);
        let tp_def = res.first().unwrap();
        assert_eq!(tp_def.id.as_str(), "PolicyProfile");
        assert_eq!(tp_def.tag_name.as_str(), "TechnicalProfile");
    }

    #[test]
    fn test_tag_info() {
        let (t, s) = get_test_str();
        let pos = Position {
            line: 16,
            character: 50,
        };
        let node = t.root_node();
        let res = get_tag_name(&node, pos);
        assert!(res.is_some());
        let n = res.unwrap();
        assert_eq!("element", n.grammar_name());
        assert_eq!(
            "<DefaultUserJourney ReferenceId=\"ProfileEdit\"/>",
            n.utf8_text(s.as_bytes()).unwrap()
        );
    }

    #[test]
    fn test_tag_query() {
        let (t, s) = get_test_str();
        let pos = Position {
            line: 17,
            character: 50,
        };
        let node = t.root_node();
        let res = get_tag_name(&node, pos);
        let n = res.unwrap();
        let tag = parse_tag(n, s.as_str());
        assert_eq!(tag.clone().unwrap().name, "TechnicalProfile");
        assert_eq!(
            tag.unwrap().attrs.get("Id"),
            Some(&String::from("PolicyProfile"))
        );
    }
}
