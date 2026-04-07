use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct InternId(pub u32);

pub struct InternTable {
    str_to_id: HashMap<String, InternId>,
    id_to_str: Vec<String>,
}

impl Default for InternTable {
    fn default() -> Self {
        Self::new()
    }
}

impl InternTable {
    pub fn new() -> Self {
        Self {
            str_to_id: HashMap::new(),
            id_to_str: Vec::new(),
        }
    }

    pub fn intern(&mut self, s: &str) -> InternId {
        if let Some(&id) = self.str_to_id.get(s) {
            return id;
        }
        let id = InternId(self.id_to_str.len() as u32);
        self.id_to_str.push(s.to_string());
        self.str_to_id.insert(s.to_string(), id);
        id
    }

    pub fn resolve(&self, id: InternId) -> &str {
        &self.id_to_str[id.0 as usize]
    }

    pub fn find(&self, s: &str) -> Option<InternId> {
        self.str_to_id.get(s).copied()
    }
}
