pub mod format;
pub mod widget;

pub use format::*;
pub use widget::*;

use crossbeam_channel::select;
use std::collections::BTreeMap;

pub struct UnixBar<F: Formatter> {
    formatter: F,
    widgets: Vec<Box<dyn Widget>>,
    fns: BTreeMap<String, Box<dyn FnMut()>>,
}

impl<F: Formatter> UnixBar<F> {
    pub fn new(formatter: F) -> UnixBar<F> {
        UnixBar {
            formatter,
            widgets: Vec::new(),
            fns: BTreeMap::new(),
        }
    }

    pub fn register_fn<Fn>(&mut self, name: &str, func: Fn) -> &mut UnixBar<F>
    where
        Fn: FnMut() + 'static,
    {
        self.fns.insert(name.to_owned(), Box::new(func));
        self
    }

    pub fn add(&mut self, widget: Box<dyn Widget>) -> &mut UnixBar<F> {
        self.widgets.push(widget);
        self
    }

    pub fn run(&mut self) {
        let (wid_tx, wid_rx) = crossbeam_channel::unbounded();
        for widget in &mut self.widgets {
            widget.spawn_notifier(wid_tx.clone());
        }
        self.show();
        let (stdin_tx, stdin_rx) = crossbeam_channel::unbounded();
        std::thread::spawn(move || {
            let stdin = std::io::stdin();
            let mut line = String::new();
            loop {
                line.clear();
                if stdin.read_line(&mut line).is_ok() {
                    stdin_tx.send(line.clone()).unwrap();
                }
            }
        });
        loop {
            select! {
                recv(wid_rx) -> _ => self.show(),
                recv(stdin_rx) -> line => self.formatter.handle_stdin(line.ok(), &mut self.fns),
            }
        }
    }

    fn show(&mut self) {
        let vals: Vec<Format> = self.widgets.iter().map(|ref w| w.current_value()).collect();
        let line = self.formatter.format_all(&vals);
        println!("{}", line.replace("\n", ""));
    }
}
