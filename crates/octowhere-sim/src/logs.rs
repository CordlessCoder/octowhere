//! The nodes' log lines, each with the node that wrote it and when. The executor names the node
//! it polls, so the node's own logging, through `log`, needs no change.

use std::{cell::RefCell, rc::Rc, sync::Once};

/// One line a node logged.
#[derive(Clone, Debug)]
pub struct Line {
    pub node: usize,
    /// Virtual time, in microseconds.
    pub at: u64,
    pub level: log::Level,
    pub text: String,
}

pub type Lines = Rc<RefCell<Vec<Line>>>;

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
