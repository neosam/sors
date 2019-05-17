use super::doc::*;
use uuid::Uuid;

#[derive(Debug)]
pub struct State {
    pub doc: Doc,
    pub wt: Uuid,
    pub parents: Vec<Uuid>,
    pub path: String
}