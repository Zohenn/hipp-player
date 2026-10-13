#[derive(serde::Serialize, serde::Deserialize, Copy, Clone)]
pub struct Seconds(u32);

impl Seconds {
    pub fn get(&self) -> u32 {
        self.0
    }
}
