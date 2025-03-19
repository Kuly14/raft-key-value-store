pub struct Config {
    pub id: u32,
    pub number_of_nodes: u32,
}
impl Config {
    pub fn new(id: u32, number_of_nodes: u32) -> Self {
        Self {
            id,
            number_of_nodes,
        }
    }
}
