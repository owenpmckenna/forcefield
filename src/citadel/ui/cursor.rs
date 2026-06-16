use std::marker::PhantomData;
use std::sync::atomic::Ordering::{AcqRel, SeqCst};
use std::sync::atomic::AtomicBool;
use tui::style::{Color, Style};
use tui::text::Span;

pub struct Cursor<T> where T: Sized {
    light: AtomicBool,
    force_light: AtomicBool,
    active: Box<dyn Fn(&T) -> bool>,
    flash_full_text: bool,
    deactive_full_text: bool
}
static UNDERLINE: &str = "_";
impl<T> Cursor<T> {
    pub fn new<A>(active: A) -> Self where A: Fn(&T) -> bool + 'static {
        Self {
            light: AtomicBool::from(true),
            force_light: AtomicBool::from(false),
            active: Box::new(active),
            flash_full_text: false,
            deactive_full_text: false
        }
    }
    pub fn set_flash_full_text(mut self) -> Self {
        self.flash_full_text = true;
        self.deactive_full_text = false;
        self
    }
    pub fn set_deactive_full_text(mut self) -> Self {
        self.deactive_full_text = true;
        self.flash_full_text = false;
        self
    }
    pub fn render<'a>(&self, mut text: Vec<Span<'a>>, data: &T) -> Vec<Span<'a>> {
        let active: bool = self.active.call((data,));
        if self.light.load(SeqCst) && active {
            if !self.flash_full_text {
                text.push(Span::styled(UNDERLINE, Style::default().fg(Color::Black).bg(Color::White)))
            } else {
                text[0].style = Style::default().fg(Color::Black).bg(Color::White);
            }
        }
        if !active && self.deactive_full_text {
            text[0].style = Style::default().fg(Color::DarkGray).bg(Color::Black);
        }
        if self.force_light.load(SeqCst) {
            self.light.store(true, SeqCst);
            self.force_light.store(false, SeqCst);
        } else {
            self.light.fetch_not(SeqCst);
        }
        text
    }
    pub fn update_key(&self) {
        self.light.store(true, SeqCst);
        self.force_light.store(true, SeqCst);
    }
}