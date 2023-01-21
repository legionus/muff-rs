use std::cmp::Ordering;
use std::env;
use std::fs::File;
use std::path::PathBuf;

use std::collections::HashMap;
use std::collections::HashSet;
use std::collections::hash_map::Values;

use mail_parser::DateTime;
use mail_parser::Message;
use mail_parser::HeaderValue;
use mail_parser::mailbox::maildir;
use mail_parser::mailbox::mbox;

struct MessageNode {
	id: String,
	parent: Option<String>,
	childs: HashSet<String>,
	message: bool,
	date: Option<DateTime>,
	subject: Option<String>,
}

struct NodeIter<'a> {
	index: usize,
	nodes: &'a NodeMap,
}

impl<'a> Iterator for NodeIter<'a> {
	type Item = &'a Box<MessageNode>;

	fn next(&mut self) -> Option<Self::Item> {
		let node = self.nodes.nth(self.index);
		self.index += 1;
		node
	}
}

struct NodeMap {
	keys: Vec<String>,
	nodes: HashMap<String, Box<MessageNode>>
}

impl NodeMap {
	fn new() -> NodeMap {
		return NodeMap {
			keys: Vec::new(),
			nodes: HashMap::new(),
		};
	}

	fn contains_key(&self, key: &str) -> bool {
		self.nodes.contains_key(key)
	}

	fn insert(&mut self, key: &str, value: Box<MessageNode>) {
		match self.nodes.insert(key.to_owned(), value) {
			None => {
				self.keys.push(key.to_owned());
			},
			_ => {},
		}
	}

	fn get(&self, key: &str) -> Option<&Box<MessageNode>> {
		self.nodes.get(key)
	}

	fn get_mut(&mut self, key: &str) -> Option<&mut Box<MessageNode>> {
		self.nodes.get_mut(key)
	}

	fn nth(&self, index: usize) -> Option<&Box<MessageNode>> {
		if let Some(x) = self.keys.get(index) {
			return self.nodes.get(x);
		}
		None
	}

	fn values(&self) -> Values<String, Box<MessageNode>> {
		self.nodes.values()
	}

	fn remove(&mut self, key: &str) -> Option<Box<MessageNode>> {
		for (i, v) in self.keys.iter().enumerate() {
			if v == key {
				self.keys.remove(i);
				break;
			}
		}
		self.nodes.remove(key)
	}

	fn sort_by<F>(&mut self, mut compare: F)
		where
			F: FnMut(&Box<MessageNode>, &Box<MessageNode>) -> Ordering,
	{
		self.keys.sort_by(|a, b| {
			let node_a = self.nodes.get(a).unwrap();
			let node_b = self.nodes.get(b).unwrap();

			compare(node_a, node_b)
		});
	}

	fn iter(&self) -> NodeIter {
		NodeIter {
			index: 0,
			nodes: self,
		}
	}
}

fn refs_append(refs: &mut Vec<String>, seen: &mut HashSet<String>, values: &HeaderValue) {
	match values {
		HeaderValue::TextList(x) => {
			for id in x {
				let ref_id = id.to_string();

				if seen.insert(ref_id.to_string()) {
					refs.push(ref_id.to_string());
				}
			}
		},
		HeaderValue::Text(x) => {
			let ref_id = x.to_string();

			if seen.insert(ref_id.to_string()) {
				refs.push(ref_id.to_string());
			}
		},
		_ => {},
	};
}

fn get_references(message: &Message) -> Vec<String> {
	let mut seen: HashSet<String> = HashSet::new();
	let mut refs: Vec<String> = Vec::new();

	refs_append(&mut refs, &mut seen, message.references());
	refs_append(&mut refs, &mut seen, message.in_reply_to());
	refs.push(message.message_id().unwrap().to_string());

	refs
}

fn create_node<'a>(message_id: &str, nodes: &'a mut NodeMap) -> &'a mut MessageNode {
	if ! nodes.contains_key(message_id) {
		nodes.insert(message_id, Box::new(MessageNode {
			id: String::from(message_id),
			parent: None,
			childs: HashSet::new(),
			message: false,
			date: None,
			subject: None,
		}));
	}
	nodes.get_mut(message_id).unwrap()
}

fn is_loop(nodes: &mut NodeMap, ref_id: &str, from_id: &str) -> bool {
	let mut cur_id = from_id;

	loop {
		if ref_id == cur_id {
			return true;
		}

		let node = nodes.get(cur_id).unwrap();

		match &node.parent {
			Some(x) => { cur_id = &x; },
			None    => { break;       },
		}
	}
	return false;
}

fn process_message(message: &Message, nodes: &mut NodeMap) {
	let message_id = message.message_id().unwrap();
	let refs = get_references(message);

	let node = create_node(&message_id, nodes);
	node.message = true;

	if let Some(x) = message.date() {
		node.date = Some(x.to_owned());
	}

	if let Some(x) = message.subject() {
		node.subject = Some(x.to_owned());
	}

	for (i, ref_id) in refs.iter().enumerate() {
		if i > 0 && is_loop(nodes, ref_id, &refs[i - 1]) {
			continue;
		}

		let node = create_node(&ref_id, nodes);

		if node.parent == None && i > 0 {
			let parent_id = &refs[i - 1];
			let node_id = String::from(&node.id);

			node.parent = Some(String::from(parent_id));

			let parent = create_node(parent_id, nodes);
			parent.childs.insert(node_id);
		}
	}
}


fn walk(nodes: &NodeMap, stack: &mut Vec<String>, is_last: usize, node: &Box<MessageNode>) {
	let cross    = &[0x251C, 0x2500];
	let corner   = &[0x2514, 0x2500];
	let vertical = &[0x2502, 0x0020];
	let space    = &[0x0020, 0x0020];

	if node.parent != None {
		for s in stack.iter() {
			print!("{}", s);
		}
		if is_last == 0 {
			print!("{}", String::from_utf16_lossy(corner));
			stack.push(String::from_utf16_lossy(space));
		} else {
			print!("{}", String::from_utf16_lossy(cross));
			stack.push(String::from_utf16_lossy(vertical));
		}
	}

	if let Some(s) = &node.subject {
		println!("{}", s);
	}

	for (i, ref_id) in node.childs.iter().enumerate() {
		if let Some(child) = nodes.get(&ref_id) {
			walk(nodes, stack, node.childs.len() - i - 1, child);
		}
	}

	if node.parent != None {
		stack.pop();
	}
}

fn main() -> std::io::Result<()> {
	let path = PathBuf::from(env::args().nth(1).expect("no path given"));
	let mut nodes = NodeMap::new();

	if path.is_dir() {
		for msg in maildir::MessageIterator::new(path)? {
			let msg = msg.unwrap();
			let message = Message::parse(msg.contents()).unwrap();

			process_message(&message, &mut nodes);
		}
	} else if path.is_file() {
		let mbox = File::open(path)?;

		for msg in mbox::MessageIterator::new(mbox) {
			let msg = msg.unwrap();
			let message = Message::parse(msg.contents()).unwrap();

			process_message(&message, &mut nodes);
		}
	}

	let mut trash: Vec<String> = Vec::new();

	for node in nodes.values() {
		if node.message {
			continue;
		}
		trash.push(node.id.to_owned());
	}

	for node_id in trash.drain(0..) {
		let node = nodes.remove(&node_id).unwrap();

		if let Some(parent_id) = node.parent {
			if let Some(parent) = nodes.get_mut(&parent_id) {
				for id in node.childs {
					parent.childs.insert(id);
				}
			}
		}
	}

	nodes.sort_by(|a, b| {
		match a.date.cmp(&b.date) {
			Ordering::Equal => a.subject.cmp(&b.subject),
			x => x
		}
	});

	let mut stack: Vec<String> = Vec::new();

	for node in nodes.iter() {
		if node.parent != None {
			continue;
		}

		walk(&nodes, &mut stack, 0, node);
	}

	Ok(())
}

// vim: tw=200 noexpandtab
