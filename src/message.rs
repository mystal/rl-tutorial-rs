use std::ops::{Deref, DerefMut};

use tcod::colors::Color;

#[derive(Serialize, Deserialize)]
pub struct Messages(Vec<(String, Color)>);

impl Messages {
    pub fn new(capacity: usize) -> Self {
        Messages(Vec::with_capacity(capacity))
    }

    pub fn message<T: Into<String>>(&mut self, message: T, color: Color) {
        // TODO: Consider using a VecDeque?
        // If the buffer is full, remove the first message to make room for the new one.
        if self.len() == self.capacity() {
            self.remove(0);
        }

        // Add the new line as a tuple, with the text and the color.
        self.push((message.into(), color));
    }
}

impl Deref for Messages {
    type Target = Vec<(String, Color)>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Messages {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
