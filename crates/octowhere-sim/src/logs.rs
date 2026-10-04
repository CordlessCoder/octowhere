//! The nodes' log lines, each with the node that wrote it and when. The executor names the node
//! it polls, so the node's own logging, through `log`, needs no change.

use std::{cell::RefCell, collections::VecDeque, rc::Rc, sync::Once};

/// One line a node logged.
#[derive(Clone, Debug)]
pub struct Line {
    pub node: usize,
    /// Virtual time, in microseconds.
    pub at: u64,
    pub level: log::Level,
    pub text: String,
}

/// What the nodes logged, oldest first. A long interactive run keeps only the latest lines.
#[derive(Default)]
pub struct Log {
    lines: VecDeque<Line>,
    /// How many lines were dropped from the front.
    dropped: usize,
    /// The most lines kept, or `None` to keep every one.
    most: Option<usize>,
}

impl Log {
    fn push(&mut self, line: Line) {
        self.lines.push_back(line);
        if self.most.is_some_and(|most| self.lines.len() > most) {
            self.lines.pop_front();
            self.dropped += 1;
        }
    }

    pub fn keep(&mut self, most: usize) {
        self.most = Some(most);
        while self.lines.len() > most {
            self.lines.pop_front();
            self.dropped += 1;
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = &Line> {
        self.lines.iter()
    }

    /// How many lines were ever logged.
    pub fn total(&self) -> usize {
        self.dropped + self.lines.len()
    }

    /// The lines kept from line number `from` on, counting every line ever logged.
    pub fn since(&self, from: usize) -> impl Iterator<Item = &Line> {
        self.lines.iter().skip(from.saturating_sub(self.dropped))
    }
}

pub type Lines = Rc<RefCell<Log>>;

thread_local! {
    /// The node being polled, the time, and where its lines go.
    static POLLING: RefCell<Option<(usize, u64, Lines)>> = const { RefCell::new(None) };
}

/// Sends what `node` logs at time `at` to `lines` while `poll` runs.
pub fn polling<T>(node: usize, at: u64, lines: &Lines, poll: impl FnOnce() -> T) -> T {
    install();
    POLLING.with(|polling| *polling.borrow_mut() = Some((node, at, lines.clone())));
    let result = poll();
    POLLING.with(|polling| *polling.borrow_mut() = None);
    result
}

struct Capture;

impl log::Log for Capture {
    fn enabled(&self, _: &log::Metadata) -> bool {
        true
    }

    fn log(&self, record: &log::Record) {
        POLLING.with(|polling| {
            if let Some((node, at, lines)) = &*polling.borrow() {
                let text = record.args().to_string();
                if std::env::var_os("OCTOWHERE_SIM_LOG").is_some() {
                    eprintln!(
                        "{:>10.3} {node} {:<5} {text}",
                        *at as f64 / 1e6,
                        record.level()
                    );
                }
                lines.borrow_mut().push(Line {
                    node: *node,
                    at: *at,
                    level: record.level(),
                    text,
                });
            }
        });
    }

    fn flush(&self) {}
}

fn install() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        log::set_logger(&Capture).expect("no other logger");
        log::set_max_level(log::LevelFilter::Debug);
    });
}
