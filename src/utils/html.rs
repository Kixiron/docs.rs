use crate::error::Result;
use failure::err_msg;
use kuchiki::{traits::TendrilSink, ElementData, NodeDataRef, NodeRef};

#[derive(Debug)]
pub struct Extracted {
    head: NodeDataRef<ElementData>,
    body: NodeDataRef<ElementData>,
}

impl Extracted {
    pub fn head_node(&self) -> String {
        serialize(self.head.as_node())
    }

    pub fn body_node(&self) -> String {
        serialize(self.body.as_node())
    }

    pub fn map_body_class<U, F: FnOnce(&str) -> U>(&self, map: F) -> Option<U> {
        self.body.attributes.borrow().get("class").map(map)
    }
}

/// Extracts the contents of the `<head>` and `<body>` tags from an HTML document, as well as the
/// classes on the `<body>` tag, if any.
pub fn extract_head_and_body(html: &[u8]) -> Result<Extracted> {
    let dom = kuchiki::parse_html().from_utf8().one(html);

    let head = dom
        .select_first("head")
        .map_err(|_| err_msg("couldn't find <head> tag in rustdoc output"))?;

    let body = dom
        .select_first("body")
        .map_err(|_| err_msg("couldn't find <body> tag in rustdoc output"))?;

    Ok(Extracted { head, body })
}

fn serialize(v: &NodeRef) -> String {
    let mut contents = Vec::new();
    for child in v.children() {
        child
            .serialize(&mut contents)
            .expect("serialization failed");
    }

    String::from_utf8(contents).expect("non utf-8 html")
}

#[cfg(test)]
mod test {
    #[test]
    fn small_html() {
        let extracted = super::extract_head_and_body(
            br#"<head><meta name="generator" content="rustdoc"></head><body class="rustdoc struct"><p>hello</p>"#
        ).unwrap();

        assert_eq!(
            &extracted.head_node(),
            r#"<meta content="rustdoc" name="generator">"#,
        );
        assert_eq!(&extracted.body_node(), "<p>hello</p>");
        assert_eq!(
            extracted.map_body_class(|c| c.to_owned()),
            Some("rustdoc struct".to_owned()),
        );
    }

    // more of an integration test
    #[test]
    fn parse_regex_html() {
        let original = std::fs::read("benches/html/struct.CaptureMatches.html").unwrap();
        let expected_head = std::fs::read_to_string("tests/regex/head.html").unwrap();
        let expected_body = std::fs::read_to_string("tests/regex/body.html").unwrap();
        let extracted = super::extract_head_and_body(&original).unwrap();

        assert_eq!(&extracted.head_node(), expected_head.trim());
        assert_eq!(&extracted.body_node(), expected_body.trim());
        assert_eq!(
            extracted.map_body_class(|c| c.to_owned()),
            Some("rustdoc struct".to_owned()),
        );
    }
}
