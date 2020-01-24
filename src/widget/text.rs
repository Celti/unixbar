use crate::widget::base::{Sender, Widget};
use crate::format::data::Format;

pub struct Text {
    text: Format,
}

impl Widget for Text {
    fn current_value(&self) -> Format {
        self.text.clone()
    }

    fn spawn_notifier(&mut self, _: Sender<()>) {}
}

impl Text {
    pub fn new(text: Format) -> Box<Text> {
        Box::new(Text { text })
    }
}
