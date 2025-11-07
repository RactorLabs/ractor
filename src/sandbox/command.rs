use anyhow::{anyhow, Result};
use quick_xml::events::{BytesEnd, BytesStart, BytesText, Event};
use quick_xml::Reader;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct CommandChild {
    pub name: String,
    pub content: String,
}

#[derive(Debug, Clone)]
pub struct CommandInvocation {
    pub name: String,
    pub attributes: HashMap<String, String>,
    pub body: Option<String>,
    pub children: Vec<CommandChild>,
}

#[derive(Default)]
struct ElementNode {
    name: String,
    attributes: HashMap<String, String>,
    text: String,
    children: Vec<ElementNode>,
}

impl ElementNode {
    fn from_start(reader: &Reader<&[u8]>, tag: &BytesStart) -> Result<Self> {
        Ok(Self {
            name: decode(reader, tag.name().as_ref())?,
            attributes: collect_attributes(reader, tag)?,
            text: String::new(),
            children: Vec::new(),
        })
    }

    fn append_text(&mut self, text: &BytesText) -> Result<()> {
        let decoded = text.unescape()?.to_string();
        if !self.text.is_empty() {
            self.text.push('\n');
        }
        self.text.push_str(decoded.trim_matches('\r'));
        Ok(())
    }
}

pub fn parse_command_xml(payload: &str) -> Result<CommandInvocation> {
    let trimmed = payload.trim();
    if trimmed.is_empty() {
        return Err(anyhow!("empty response"));
    }

    let mut reader = Reader::from_str(trimmed);
    reader.trim_text(false);

    let mut buf = Vec::new();
    let mut stack: Vec<ElementNode> = Vec::new();
    let mut root: Option<ElementNode> = None;
    let mut finished = false;

    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Start(tag) => {
                let node = ElementNode::from_start(&reader, &tag)?;
                stack.push(node);
            }
            Event::Empty(tag) => {
                let node = ElementNode::from_start(&reader, &tag)?;
                if let Some(parent) = stack.last_mut() {
                    parent.children.push(node);
                } else if root.is_none() {
                    root = Some(node);
                    finished = true;
                } else {
                    return Err(anyhow!("multiple root elements detected"));
                }
            }
            Event::Text(text) => {
                if let Some(current) = stack.last_mut() {
                    current.append_text(&text)?;
                } else if !text.unescape()?.trim().is_empty() {
                    return Err(anyhow!("unexpected text outside root element"));
                }
            }
            Event::CData(text) => {
                if let Some(current) = stack.last_mut() {
                    let decoded = reader.decoder().decode(text.as_ref())?.to_string();
                    if !current.text.is_empty() {
                        current.text.push('\n');
                    }
                    current.text.push_str(&decoded);
                } else if !reader.decoder().decode(text.as_ref())?.trim().is_empty() {
                    return Err(anyhow!("unexpected text outside root element"));
                }
            }
            Event::End(BytesEnd { .. }) => {
                let node = stack
                    .pop()
                    .ok_or_else(|| anyhow!("unexpected closing tag"))?;
                if let Some(parent) = stack.last_mut() {
                    parent.children.push(node);
                } else if root.is_none() {
                    root = Some(node);
                    finished = true;
                } else {
                    return Err(anyhow!("multiple root elements detected"));
                }
            }
            Event::Comment(_) | Event::PI(_) | Event::Decl(_) | Event::DocType(_) => {}
            Event::Eof => break,
        }
        buf.clear();
    }

    if !stack.is_empty() {
        return Err(anyhow!("unbalanced XML tags"));
    }
    if !finished {
        return Err(anyhow!("incomplete XML command"));
    }

    // Ensure no trailing non-whitespace content
    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Eof => break,
            Event::Text(text) => {
                if !text.unescape()?.trim().is_empty() {
                    return Err(anyhow!(
                        "unexpected trailing content after command invocation"
                    ));
                }
            }
            Event::CData(text) => {
                if !reader.decoder().decode(text.as_ref())?.trim().is_empty() {
                    return Err(anyhow!(
                        "unexpected trailing content after command invocation"
                    ));
                }
            }
            Event::Comment(_) | Event::PI(_) | Event::Decl(_) | Event::DocType(_) => {}
            _ => return Err(anyhow!("unexpected trailing XML after command invocation")),
        }
        buf.clear();
    }

    let root = root.ok_or_else(|| anyhow!("no XML element found"))?;
    let body = if root.text.trim().is_empty() {
        None
    } else {
        Some(root.text.trim().to_string())
    };
    let mut children = Vec::new();
    for child in root.children {
        if !child.children.is_empty() {
            return Err(anyhow!("nested child elements are not supported"));
        }
        children.push(CommandChild {
            name: child.name,
            content: child.text.trim().to_string(),
        });
    }

    Ok(CommandInvocation {
        name: root.name,
        attributes: root.attributes,
        body,
        children,
    })
}

fn decode(reader: &Reader<&[u8]>, raw: &[u8]) -> Result<String> {
    Ok(reader.decoder().decode(raw)?.to_string())
}

fn collect_attributes(reader: &Reader<&[u8]>, tag: &BytesStart) -> Result<HashMap<String, String>> {
    let mut attrs = HashMap::new();
    for attr in tag.attributes() {
        let attr = attr?;
        let key = decode(reader, attr.key.as_ref())?;
        let value = attr.unescape_value()?.to_string();
        attrs.insert(key, value);
    }
    Ok(attrs)
}
