#![warn(warnings)]

use rlua;
use rlua::prelude::*;
use rlua::{Lua, UserData, UserDataMethods};
use select;
use select::node::Raw;
use std::mem;
use std::sync::Arc;

#[derive(Serialize, Deserialize)]
enum Predicate {
    Any,
    Text,
    Element,
    Comment,
    Class(String),
    Name(String),
    Attr(String, Option<String>),
    Not(Box<Predicate>),
    Or(Box<Predicate>, Box<Predicate>),
    And(Box<Predicate>, Box<Predicate>),
    Child(Box<Predicate>, Box<Predicate>),
    Descendant(Box<Predicate>, Box<Predicate>),
}

impl<'a> select::predicate::Predicate for &'a Predicate {
    fn matches(&self, node: &select::node::Node<'_>) -> bool {
        <Predicate as select::predicate::Predicate>::matches(self, node)
    }
}

impl select::predicate::Predicate for Predicate {
    fn matches(&self, node: &select::node::Node<'_>) -> bool {
        use self::Predicate::*;
        match self {
            Any => select::predicate::Any.matches(node),
            Text => select::predicate::Text.matches(node),
            Element => select::predicate::Element.matches(node),
            Comment => select::predicate::Comment.matches(node),
            Class(s) => select::predicate::Class(s.as_str()).matches(node),
            Name(s) => select::predicate::Name(s.as_str()).matches(node),
            Attr(s, op) => match op {
                Some(ss) => select::predicate::Attr(s.as_str(), ss.as_str()).matches(node),
                None => select::predicate::Attr(s.as_str(), ()).matches(node),
            },
            Not(pred) => select::predicate::Not(pred.as_ref()).matches(node),
            And(a, b) => select::predicate::And(a.as_ref(), b.as_ref()).matches(node),
            Or(a, b) => select::predicate::Or(a.as_ref(), b.as_ref()).matches(node),
            Child(a, b) => select::predicate::Child(a.as_ref(), b.as_ref()).matches(node),
            Descendant(a, b) => select::predicate::Descendant(a.as_ref(), b.as_ref()).matches(node),
        }
    }
}

struct Node {
    document: Document,
    index: usize,
}

impl Node {
    fn to_node<'a>(&'a self) -> select::node::Node<'a> {
        select::node::Node::new(&self.document.0, self.index).unwrap()
    }
    fn parent(&self) -> Option<Self> {
        self.to_node().parent().map(|p| Node {
            document: self.document.clone(),
            index: p.index(),
        })
    }
}

impl UserData for Node {
    fn add_methods<'lua, M: UserDataMethods<'lua, Self>>(methods: &mut M) {
        methods.add_method("text", |_, this, _: ()| Ok(this.to_node().text()));

        methods.add_method("html", |_, this, _: ()| Ok(this.to_node().html()));

        methods.add_method("parent", |_, this, _: ()| Ok(this.parent()));

        methods.add_method("find", |_, this, val: rlua::Value| {
            let pred: Predicate = rlua_serde::from_value(val)?;
            let vec: Vec<_> = this
                .to_node()
                .find(pred)
                .map(|node| Node {
                    document: this.document.clone(),
                    index: node.index(),
                }).collect();
            Ok(vec)
        });
    }
}

#[derive(Clone)]
struct Document(Arc<select::document::Document>);

fn into_send(raw: &mut Raw) {
    use select::node::Data;
    match raw.data {
        Data::Text(ref mut tendril) => {
            *tendril = unsafe { mem::transmute(tendril.clone().into_send()) };
        }
        Data::Comment(ref mut tendril) => {
            *tendril = unsafe { mem::transmute(tendril.clone().into_send()) };
        }
        Data::Element(_, ref mut vec) => {
            for (_, tendril) in vec {
                *tendril = unsafe { mem::transmute(tendril.clone().into_send()) };
            }
        }
    }
}

impl Document {
    fn from_str(text: &str) -> Document {
        let mut doc = select::document::Document::from(text);
        for raw in &mut doc.nodes {
            into_send(raw);
        }
        Document(Arc::new(doc))
    }
}

unsafe impl Send for Document {}

impl UserData for Document {
    fn add_methods<'lua, M: UserDataMethods<'lua, Self>>(methods: &mut M) {
        methods.add_method("find", |_, this, val: rlua::Value| {
            let pred: Predicate = rlua_serde::from_value(val)?;
            let vec: Vec<_> = this
                .0
                .find(pred)
                .map(|node| Node {
                    document: this.clone(),
                    index: node.index(),
                }).collect();
            Ok(vec)
        });
    }
}

pub fn init(lua: &Lua) -> Result<(), LuaError> {
    let select = lua.create_table()?;

    // New Document from string
    select.set(
        "document",
        lua.create_function(|_, text: String| Ok(Document::from_str(text.as_str())))?,
    )?;

    select.set(
        "name",
        lua.create_function(|lua, text: String| {
            let pred = Predicate::Name(text);
            rlua_serde::to_value(lua, &pred)
        })?,
    )?;

    let globals = lua.globals();
    globals.set("select", select)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use rlua::prelude::*;
    use rlua::{FromLua, Lua, MetaMethod, Table, UserData, UserDataMethods, Value};

    #[test]
    fn test() {
        let lua = Lua::new();
        super::init(&lua).unwrap();
        lua.exec::<_, Value>(
            r#"
        local doc = select.document("<p>hello</p>")
        local vec = doc:find(select.name("p"))
        assert(vec[1]:text() == "hello")
        "#,
            None,
        ).unwrap();
    }
}
