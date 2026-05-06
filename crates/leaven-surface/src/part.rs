//! Surface parts.

pub struct Part<Id, Address, View> {
    pub id: Id,
    pub address: Address,
    pub kind: PartKind,
    pub view: View,
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum PartKind {
    Text,
    File,
    Directory,
    Prompt,
    Skill,
    Script,
    Config,
    CodeModule,
    ConflictRegion,
    Changeset,
    Agent,
    Opaque,
}

pub struct PartView<T> {
    pub inner: T,
}
