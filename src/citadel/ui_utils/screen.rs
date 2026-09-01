use crossterm::event::{KeyCode, KeyEvent};
use std::io::Stdout;
use std::ops::Range;
use std::time::{SystemTime, UNIX_EPOCH};
use tui::Frame;
use tui::backend::CrosstermBackend;
use tui::layout::{Alignment, Rect};
use tui::style::{Color, Style};
use tui::text::{Span, Spans};
use tui::widgets::{Block, BorderType, Borders, Paragraph, Wrap};
use crate::citadel::state::BackendState;

///This is a relatively simple tui setup that provides support for what I need: buttons, text input, text, and multiple panes.
pub struct Screen<T> {
	pub panes: Vec<Pane<T>>,
	pane_selected: usize,
	run_yet: bool
}
impl<T> Screen<T> {
	pub fn new(panes: Vec<Pane<T>>) -> Option<Self> {
		if panes.is_empty() {
			return None
		}
		Some(Self {
			panes,
			pane_selected: 0,
			run_yet: false
		})
	}
	pub fn render(&mut self, frame: &mut Frame<CrosstermBackend<Stdout>>, layout: Vec<Rect>, owner: &mut T, state: &BackendState) -> Option<()> {
		if !self.run_yet {
			self.run_yet = true;
			self.pane().on_select(owner);
		}
		if layout.len() != self.panes.len() {
			return None
		}
		for i in 0..layout.len() {
			self.panes[i].render(frame, layout[i], i == 0, owner, state);
		}
		Some(())
	}
	fn pane(&mut self) -> &mut Pane<T> {
		&mut self.panes[self.pane_selected]
	}
	pub fn on_key(&mut self, key_event: KeyEvent, owner: &mut T, backend: &mut BackendState) {
		match key_event.code {
			KeyCode::Tab => {
				self.pane().on_deselect(owner);
				self.pane_selected = (self.pane_selected + 1) % self.panes.len();
				self.pane().on_select(owner);
			}
		    _ => {
				self.pane().on_key(key_event, owner, backend);
			},
		}
	}
}
pub struct Pane<T> {
	title: String,
	pub elements: Vec<Box<dyn Element<T>>>,
	element_selected: usize,
	pub render_as_list: bool,
	pub last_button: bool,
	pub last_line_separated: bool,
	///from, to
	pub enter_redirectors: Vec<(usize, usize)>,
	pub allow_updown: bool,
	selected: bool
}
impl<T> Pane<T> {
	pub fn new(title: &str, elements: Vec<Box<dyn Element<T>>>) -> Self {
		Self {
			title: title.to_string(),
			elements,
			element_selected: 0,
			render_as_list: false,
			last_button: false,
			last_line_separated: false,
			enter_redirectors: vec![],
			allow_updown: true,
			selected: false
		}
	}
	fn on_select(&mut self, owner: &mut T) {
		self.selected = true;
		self.element().on_select(owner);
	}
	fn on_deselect(&mut self, owner: &mut T) {
		self.selected = false;
		self.element().on_deselect(owner);
	}
	fn render<'a: 'b, 'b>(&'b mut self, frame: &mut Frame<CrosstermBackend<Stdout>>, layout: Rect, top_left: bool, owner: &'a mut T, state: &BackendState) {
		let width = if self.render_as_list {layout.width - 2} else {layout.width};
		let elements_len = self.elements.len();
		let mut out = self.elements.iter_mut().enumerate().map(|(ind, it)| {
			Spans::from(it.render(width, owner, state))
		}).collect::<Vec<_>>();
		if self.render_as_list {
			out.iter_mut().enumerate().for_each(|(i, it)| {
				//if it's selected and not the button of the list
				let txt = if self.element_selected == i && !(self.last_button && i == elements_len - 1) && self.selected {
					"> "
				} else {"  "};
				it.0.insert(0, Span::raw(txt));
			});
		}
		if self.last_line_separated && out.len() < layout.height as usize {
			let to_insert = layout.height - out.len() as u16;
			let index = out.len() - 1;
			for i in 0..to_insert {
				out.insert(index, Span::raw("").into())
			}
		}
		let paragraph = Paragraph::new(out)
			.style(Style::default().fg(Color::White).bg(Color::Black))
			.alignment(Alignment::Left)
			.block(
				Block::default()
					.border_type(BorderType::Plain)
					//TODO borders is completely broken rn
					.borders( if top_left {Borders::all()} else {Borders::BOTTOM | Borders::RIGHT})
					.title(self.title.clone())
			)
			.wrap(Wrap { trim: false });
		frame.render_widget(paragraph, layout);
	}
	fn element(&mut self) -> &mut Box<dyn Element<T>> {
		&mut self.elements[self.element_selected]
	}
	fn select(&mut self, diff: isize, owner: &mut T) {
		if self.elements.len() == 1 {
			return;
		}
		self.elements[self.element_selected].on_deselect(owner);
		let len = self.elements.len() as isize + diff;
		self.element_selected = (self.element_selected + len as usize) % self.elements.len();
		self.elements[self.element_selected].on_select(owner);
	}
	fn on_key(&mut self, key_event: KeyEvent, owner: &mut T, state: &mut BackendState) {
		let updown = self.allow_updown as isize & 1;
		match &key_event.code {
			KeyCode::Up => {
				self.select(-1 * updown, owner);
			},
			KeyCode::Down => {
				self.select(1 * updown, owner);
			},
			KeyCode::Enter => {
				let elem = self.enter_redirectors.iter().find_map(|(from, to)| {
					if *from == self.element_selected {
						Some(*to)
					} else {None}
				}).unwrap_or(self.element_selected);
				self.elements[elem].on_key(key_event, owner, state);
			}
		    _ => {
				if self.element().on_key(key_event, owner, state) {
					self.elements.remove(self.element_selected);
				}
			}
		}
	}
}
pub trait Element<T> {
	fn on_select(&mut self, _: &mut T) {}
	fn on_deselect(&mut self, _: &mut T) {}
	fn on_key(&mut self, key_event: KeyEvent, _: &mut T, _: &mut BackendState) -> bool;
	fn render<'a: 'b, 'b: 'c, 'c>(&'b mut self, width: u16, _: &'a T, _: &'a BackendState) -> Vec<Span<'c>>;
}
type Action<E, T> = Box<dyn FnMut(&mut E, &mut T, &mut BackendState)>;
pub struct ButtonElement<T> {
	allow_highlighting: Option<fn(&T) -> bool>,
	selected: bool,
	pub text: String,
	center: bool,
	on_click: Option<Action<Self, T>>,
	pub other_text: Option<Box<dyn for<'a> FnMut(&'a T, &'a BackendState) -> &'a str>>,
	pub marked_for_removal: bool
}
impl<T> ButtonElement<T> {
	pub fn new_<A>(text: &str, center: bool, on_click: A) -> Self where A: FnMut(&mut Self, &mut T, &mut BackendState) + 'static {
		Self::new(text, Some(|_| true), center, on_click)
	}
	pub fn new<A>(text: &str, allow_highlighting: Option<fn(&T) -> bool>, center: bool, on_click: A) -> Self where A: FnMut(&mut Self, &mut T, &mut BackendState) + 'static {
		Self {
			selected: false,
			text: text.to_string(),
			allow_highlighting,
			center,
			on_click: Some(Box::new(on_click)),
			other_text: None,
			marked_for_removal: false
		}
	}
}
impl<T> Element<T> for ButtonElement<T> {
	fn on_select(&mut self, _: &mut T) {
		self.selected = true;
	}
	fn on_deselect(&mut self, _: &mut T) {
		self.selected = false;
	}
	fn on_key(&mut self, key_event: KeyEvent, owner: &mut T, state: &mut BackendState) -> bool {
		if key_event.code == KeyCode::Enter {
			let mut taken = self.on_click.take().unwrap();
			taken(self, owner, state);
			let _ = self.on_click.insert(taken);
		}
		self.marked_for_removal
	}

	fn render<'a: 'b, 'b: 'c, 'c>(&'b mut self, width: u16, owner: &'a T, state: &'a BackendState) -> Vec<Span<'c>> {
		let mut style = Style::default();
		if self.selected && self.allow_highlighting.map(|it| it(owner)).unwrap_or(true) {
			style = style.fg(Color::LightBlue);
		}
		let mut vec = if let Some(it) = &mut self.other_text {
			vec![Span::styled(&self.text, style), Span::styled(it(owner, state), style)]
		} else {
			vec![Span::styled(&self.text, style)]
		};
		if self.center {
			center_text(width, &mut vec);
		}
		vec
	}
}
fn center_text(width: u16, span: &mut Vec<Span>) {
	let size = span.iter().map(Span::width).sum::<usize>() as u16;
	if size >= width {
		return;
	}
	let buf = " ".repeat(((width - size) / 2) as usize);
	span.insert(0, Span::raw(buf));
}
//TODO handle more text than can fit in the box
pub struct TextInputElement<T> {
	pub title: String,
	txt_buffer: Box<dyn for<'a> FnMut(&'a mut T, &'a mut BackendState) -> &'a mut String>,
	txt_buffer_view: Box<dyn for<'a> FnMut(&'a T, &'a BackendState) -> &'a str>,
	cursor: usize,
	pub cursor_flash_rate: u128,
	selected: bool,
	pub on_enter: Option<Box<dyn FnMut(&mut T, &mut BackendState)>>
}
impl<T> TextInputElement<T> {
	pub fn new<A, B>(title: &str, txt_buffer: A, txt_buffer_view: B) -> Self where A: for<'a> FnMut(&'a mut T, &'a mut BackendState) -> &'a mut String + 'static, 
																				   B: for<'a> FnMut(&'a T, &'a BackendState) -> &'a str + 'static{
		Self {
			title: title.to_string(),
			txt_buffer: Box::new(txt_buffer),
			txt_buffer_view: Box::new(txt_buffer_view),
			cursor: 0,
			cursor_flash_rate: 1000,
			selected: false,
			on_enter: None
		}
	}
	fn get_indexes(txt: &String, pos: usize) -> Option<Range<usize>> {
		let mut ind = txt.char_indices();
		let (first, _) = ind.nth(pos)?;
		let second = if let Some((second, _)) = ind.next() {
			second
		} else {
			txt.len()
		};
		Some(first..second)
	}
}
impl<T> Element<T> for TextInputElement<T> {
	fn on_select(&mut self, _: &mut T) {
		self.selected = true;
	}
	fn on_deselect(&mut self, _: &mut T) {
		self.selected = false;
	}
	fn on_key(&mut self, key_event: KeyEvent, owner: &mut T, state: &mut BackendState) -> bool {
		let txt = (self.txt_buffer)(owner, state);
		match key_event.code {
			KeyCode::Char(a) => {
				if let Some((index, _)) = txt.char_indices().nth(self.cursor) {
					txt.insert(index, a);
				} else {
					txt.push(a);
				}
				self.cursor += 1;
			},
			KeyCode::Backspace => {
				if !txt.is_empty() && self.cursor != 0 && let Some(range) = Self::get_indexes(txt, self.cursor - 1) {
					for _ in 0..range.end - range.start {
						txt.remove(range.start);
					}
					self.cursor -= 1;
				}
			},
			KeyCode::Left => {
				self.cursor = self.cursor.saturating_sub(1);
			},
			KeyCode::Right => {
				self.cursor = (self.cursor + 1).min(txt.chars().count())
			},
			KeyCode::Enter => {
				if let Some(enter) = &mut self.on_enter {
					enter(owner, state)
				}
			},
			_ => {}
		}
		false
	}

	fn render<'a: 'b, 'b: 'c, 'c>(&'b mut self, width: u16, owner: &'a T, state: &'a BackendState) -> Vec<Span<'c>> {
		let text = (self.txt_buffer_view)(owner, state);
		let txt_len = text.chars().count();
		let def_style = Style::default();
		let epoch = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().subsec_millis();
		let cursor = if epoch > 500 {
			Style::default().fg(Color::Black).bg(Color::White)
		} else {
			def_style
		};
		if !self.selected {
			vec![Span::styled(&self.title, def_style), Span::styled(text, def_style)]
		} else if self.cursor == txt_len {
			[
				Span::styled(&self.title, def_style),
				Span::styled(text, def_style),
				Span::styled(if epoch > 500 {"█"} else {" "}, def_style)
			].into_iter().collect()
		} else if self.cursor == txt_len - 1 {
			[
				Some(Span::styled(&self.title, def_style)),
				optional_span(text, 0, Some(self.cursor), def_style),
				optional_span(text, self.cursor, None, cursor),
			].into_iter().flatten().collect()
		} else {
			[
				Some(Span::styled(&self.title, def_style)),
				optional_span(text, 0, Some(self.cursor), def_style),
				optional_span(text, self.cursor, Some(self.cursor + 1), cursor),
				optional_span(text, self.cursor + 1, None, def_style),
			].into_iter().flatten().collect()
		}
	}
}
fn optional_span(str: &str, from: usize, to: Option<usize>, style: Style) -> Option<Span<'_>> {
	//just checking if from and to are the same in all cases (eg. str.len() is 0 or to is None)
	if let Some(pos) = to && pos == from {
		return None
	}
	if str.is_empty() {
		return None
	}

	let start = char_pos(str, from)?;
	let str = if let Some(to) = to {
		let end = char_pos(str, to)?;
		&str[start..end]
	} else {
		&str[start..]
	};
	Some(Span::styled(str, style))
}
fn char_pos(str: &str, at: usize) -> Option<usize> {
	str.char_indices().nth(at).map(|it| it.0)
}
pub trait TextViewer<A> = for<'a> FnMut(&'a A, &'a BackendState) -> &'a str + 'static;
pub struct TextView<T> {
	txt_buffer_view: Vec<Box<dyn TextViewer<T>>>,
	highlightable: bool,
	selected: bool
}
impl<T> TextView<T> {//for<'a> FnMut(&'a T, &'a BackendState) -> &'a str + 'static
	pub fn new<A: TextViewer<T>>(txt_buffer_view: A, highlightable: bool) -> Self {
		Self {
			txt_buffer_view: vec![Box::new(txt_buffer_view)], highlightable, selected: false
		}
	}
	pub fn add_new<A: TextViewer<T>>(mut self, buffer_view: A) -> TextView<T> {
		self.txt_buffer_view.push(Box::new(buffer_view));
		self
	}
}
impl<T> Element<T> for TextView<T> {
	fn on_select(&mut self, _: &mut T) {
		self.selected = self.highlightable;
	}
	fn on_deselect(&mut self, _: &mut T) {
		self.selected = false;
	}
	fn on_key(&mut self, _: KeyEvent, _: &mut T, _: &mut BackendState) -> bool {false}

	fn render<'a: 'b, 'b: 'c, 'c>(&'b mut self, width: u16, owner: &'a T, state: &'a BackendState) -> Vec<Span<'c>> {
		self.txt_buffer_view.iter_mut().map(|it| Span::styled(it(owner, state), Style::default())).collect()
	}
}
#[macro_export] macro_rules! bvec {
    ($($item:expr),* $(,)?) => {
        vec![
            $(Box::new($item)),*
        ]
    };
}