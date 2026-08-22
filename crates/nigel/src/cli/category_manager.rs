use crossterm::event::KeyCode;
use ratatui::{
    layout::{Constraint, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};
use rusqlite::Connection;

use crate::cli::categories::{self, CategoryRow};
use crate::tui::{FOOTER_STYLE, HEADER_STYLE};
use nigel_core::db::AccountClass;

const CATEGORY_TYPES: &[&str] = &["expense", "income"];

fn class_options() -> Vec<String> {
    AccountClass::ALL
        .iter()
        .map(|c| c.as_str().to_string())
        .collect()
}

fn class_field(selected: AccountClass) -> FormField {
    let options = class_options();
    let index = options
        .iter()
        .position(|o| o == selected.as_str())
        .unwrap_or(0);
    FormField {
        label: "Class",
        value: selected.as_str().to_string(),
        kind: FieldKind::Selector {
            options,
            selected: index,
        },
    }
}

// Field indices for CategoryForm — keep in sync with field order
const NAME_IDX: usize = 0;
const TYPE_IDX: usize = 1;
const CLASS_IDX: usize = 2;
const TAX_LINE_IDX: usize = 3;
const FORM_LINE_IDX: usize = 4;

pub enum CategoryAction {
    Continue,
    Close,
}

enum Screen {
    List,
    Add(CategoryForm),
    Edit(CategoryForm),
    ConfirmDelete,
}

struct CategoryForm {
    fields: Vec<FormField>,
    focused: usize,
    /// Whether the operator moved the class selector.
    ///
    /// The field has to open on something, and what it opens on is a default
    /// rather than an answer. An add sends `None` until this is true, so
    /// `add_category` derives the class from the type — otherwise every income
    /// category added without touching the field would be filed as an expense.
    class_chosen: bool,
}

struct FormField {
    label: &'static str,
    value: String,
    kind: FieldKind,
}

enum FieldKind {
    Text,
    Selector {
        options: Vec<String>,
        selected: usize,
    },
}

impl CategoryForm {
    fn new_add() -> Self {
        Self {
            fields: vec![
                FormField {
                    label: "Name",
                    value: String::new(),
                    kind: FieldKind::Text,
                },
                FormField {
                    label: "Type",
                    value: CATEGORY_TYPES[0].to_string(),
                    kind: FieldKind::Selector {
                        options: CATEGORY_TYPES.iter().map(|s| s.to_string()).collect(),
                        selected: 0,
                    },
                },
                class_field(AccountClass::Expense),
                FormField {
                    label: "Tax Line",
                    value: String::new(),
                    kind: FieldKind::Text,
                },
                FormField {
                    label: "Form Line",
                    value: String::new(),
                    kind: FieldKind::Text,
                },
            ],
            focused: 0,
            class_chosen: false,
        }
    }

    fn new_edit(cat: &CategoryRow) -> Self {
        let type_idx = CATEGORY_TYPES
            .iter()
            .position(|t| *t == cat.category_type)
            .unwrap_or(0);
        Self {
            fields: vec![
                FormField {
                    label: "Name",
                    value: cat.name.clone(),
                    kind: FieldKind::Text,
                },
                FormField {
                    label: "Type",
                    value: cat.category_type.clone(),
                    kind: FieldKind::Selector {
                        options: CATEGORY_TYPES.iter().map(|s| s.to_string()).collect(),
                        selected: type_idx,
                    },
                },
                class_field(cat.class),
                FormField {
                    label: "Tax Line",
                    value: cat.tax_line.clone().unwrap_or_default(),
                    kind: FieldKind::Text,
                },
                FormField {
                    label: "Form Line",
                    value: cat.form_line.clone().unwrap_or_default(),
                    kind: FieldKind::Text,
                },
            ],
            focused: 0,
            class_chosen: true,
        }
    }
}

pub struct CategoryManager {
    categories: Vec<CategoryRow>,
    selection: usize,
    scroll_offset: usize,
    last_visible_rows: usize,
    screen: Screen,
    status_message: Option<String>,
    /// Remaining keypresses before the status message is cleared.
    status_ttl: u8,
    greeting: String,
}

impl CategoryManager {
    pub fn new(conn: &Connection, greeting: &str) -> Self {
        let categories = categories::list_categories(conn).unwrap_or_default();
        Self {
            categories,
            selection: 0,
            scroll_offset: 0,
            last_visible_rows: 20,
            screen: Screen::List,
            status_message: None,
            status_ttl: 0,
            greeting: greeting.to_string(),
        }
    }

    fn reload(&mut self, conn: &Connection) {
        self.categories = categories::list_categories(conn).unwrap_or_default();
        if !self.categories.is_empty() {
            self.selection = self.selection.min(self.categories.len() - 1);
        } else {
            self.selection = 0;
        }
    }

    pub fn draw(&mut self, frame: &mut Frame) {
        match &self.screen {
            Screen::List | Screen::ConfirmDelete => self.draw_list(frame),
            Screen::Add(form) => self.draw_form(frame, "Add Category", form),
            Screen::Edit(form) => self.draw_form(frame, "Edit Category", form),
        }
    }

    fn draw_list(&mut self, frame: &mut Frame) {
        let area = frame.area();
        let border_style = Style::default().fg(Color::DarkGray);

        let [header_area, sep, content_area, hints_area] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Fill(1),
            Constraint::Length(1),
        ])
        .areas(area);

        // Header
        frame.render_widget(
            Paragraph::new(format!(" Nigel: {}", self.greeting)).style(HEADER_STYLE),
            header_area,
        );

        let sep_line = "\u{2501}".repeat(area.width as usize);
        frame.render_widget(Paragraph::new(sep_line.as_str()).style(border_style), sep);

        // Content: title + table
        let visible_height = content_area.height as usize;
        // 3 lines for title area + 1 for column header = 4 lines overhead
        let data_rows = visible_height.saturating_sub(4);
        self.last_visible_rows = data_rows;

        let mut lines = vec![
            Line::from(""),
            Line::from(Span::styled(
                format!(" Chart of Accounts ({})", self.categories.len()),
                Style::default().add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
        ];

        if self.categories.is_empty() {
            lines.push(Line::from("   No categories. Press 'a' to add one."));
        } else {
            // Table header — column widths are 28, 10, 20; truncation is width-2
            // to leave room for the ellipsis character and padding.
            lines.push(Line::from(vec![
                Span::styled("   ", Style::default()),
                Span::styled(
                    format!(
                        "{:<28} {:<10} {:<10} {:<16} {}",
                        "Name", "Type", "Class", "Tax Line", "Form Line"
                    ),
                    Style::default()
                        .fg(Color::DarkGray)
                        .add_modifier(Modifier::BOLD),
                ),
            ]));

            let end = (self.scroll_offset + data_rows).min(self.categories.len());
            for i in self.scroll_offset..end {
                let cat = &self.categories[i];
                let marker = if i == self.selection { " > " } else { "   " };
                let style = if i == self.selection {
                    Style::default().add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };
                let tax = cat.tax_line.as_deref().unwrap_or("");
                let form = cat.form_line.as_deref().unwrap_or("");
                lines.push(Line::from(Span::styled(
                    format!(
                        "{marker}{:<28} {:<10} {:<10} {:<16} {}",
                        truncate(&cat.name, 26),
                        cat.category_type,
                        cat.class.as_str(),
                        truncate(tax, 14),
                        form
                    ),
                    style,
                )));
            }
        }

        // Delete confirmation inline
        if let Screen::ConfirmDelete = &self.screen {
            if let Some(cat) = self.categories.get(self.selection) {
                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled(
                    format!("   Delete '{}'? (y/n)", cat.name),
                    Style::default().fg(Color::Yellow),
                )));
            }
        }

        frame.render_widget(Paragraph::new(lines), content_area);

        // Hints / status
        if let Some(msg) = &self.status_message {
            frame.render_widget(
                Paragraph::new(format!(" {msg}")).style(Style::default().fg(Color::Yellow)),
                hints_area,
            );
        } else if let Screen::ConfirmDelete = &self.screen {
            frame.render_widget(
                Paragraph::new(" y=confirm  n=cancel").style(FOOTER_STYLE),
                hints_area,
            );
        } else {
            frame.render_widget(
                Paragraph::new(" a=add  e=edit  d=delete  Esc=back  q=quit").style(FOOTER_STYLE),
                hints_area,
            );
        }
    }

    fn draw_form(&self, frame: &mut Frame, title: &str, form: &CategoryForm) {
        let area = frame.area();
        let border_style = Style::default().fg(Color::DarkGray);

        let [header_area, sep, content_area, hints_area] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Fill(1),
            Constraint::Length(1),
        ])
        .areas(area);

        frame.render_widget(
            Paragraph::new(format!(" Nigel: {}", self.greeting)).style(HEADER_STYLE),
            header_area,
        );

        let sep_line = "\u{2501}".repeat(area.width as usize);
        frame.render_widget(Paragraph::new(sep_line.as_str()).style(border_style), sep);

        let mut lines = vec![
            Line::from(""),
            Line::from(Span::styled(
                format!(" {title}"),
                Style::default().add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
        ];

        for (i, field) in form.fields.iter().enumerate() {
            let is_focused = i == form.focused;
            let label_style = if is_focused {
                Style::default().add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            match &field.kind {
                FieldKind::Text => {
                    let cursor = if is_focused { "_" } else { "" };
                    lines.push(Line::from(vec![
                        Span::styled(format!("   {:<14} ", field.label), label_style),
                        Span::styled(
                            format!("{}{cursor}", field.value),
                            if is_focused {
                                Style::default().fg(Color::Cyan)
                            } else {
                                Style::default()
                            },
                        ),
                    ]));
                }
                FieldKind::Selector { options, selected } => {
                    let arrows = if is_focused {
                        ("< ", " >")
                    } else {
                        ("  ", "  ")
                    };
                    lines.push(Line::from(vec![
                        Span::styled(format!("   {:<14} ", field.label), label_style),
                        Span::styled(
                            format!("{}{}{}", arrows.0, options[*selected], arrows.1),
                            if is_focused {
                                Style::default().fg(Color::Cyan)
                            } else {
                                Style::default()
                            },
                        ),
                    ]));
                }
            }
        }

        // Status message below form
        if let Some(msg) = &self.status_message {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                format!("   {msg}"),
                Style::default().fg(Color::Yellow),
            )));
        }

        frame.render_widget(Paragraph::new(lines), content_area);

        frame.render_widget(
            Paragraph::new(" Tab=next field  Enter=save  Esc=cancel").style(FOOTER_STYLE),
            hints_area,
        );
    }

    fn set_status(&mut self, msg: String) {
        self.status_message = Some(msg);
        self.status_ttl = 3;
    }

    fn ensure_visible(&mut self, visible_rows: usize) {
        if self.selection < self.scroll_offset {
            self.scroll_offset = self.selection;
        } else if self.selection >= self.scroll_offset + visible_rows {
            self.scroll_offset = self.selection - visible_rows + 1;
        }
    }

    pub fn handle_key(&mut self, code: KeyCode, conn: &Connection) -> CategoryAction {
        // TTL counts remaining keypresses after the one that set the status.
        // A set_status(ttl=3) means the message survives 3 more keypresses.
        if self.status_ttl > 0 {
            self.status_ttl -= 1;
            if self.status_ttl == 0 {
                self.status_message = None;
            }
        }

        match &mut self.screen {
            Screen::List => self.handle_list_key(code, conn),
            Screen::Add(_) => self.handle_form_key(code, conn, FormMode::Add),
            Screen::Edit(_) => self.handle_form_key(code, conn, FormMode::Edit),
            Screen::ConfirmDelete => self.handle_delete_key(code, conn),
        }
    }

    fn handle_list_key(&mut self, code: KeyCode, conn: &Connection) -> CategoryAction {
        match code {
            KeyCode::Up => {
                self.selection = self.selection.saturating_sub(1);
                self.ensure_visible(self.last_visible_rows);
            }
            KeyCode::Down => {
                if !self.categories.is_empty() {
                    self.selection = (self.selection + 1).min(self.categories.len() - 1);
                    self.ensure_visible(self.last_visible_rows);
                }
            }
            KeyCode::Char('a') => {
                self.screen = Screen::Add(CategoryForm::new_add());
            }
            KeyCode::Char('e') => {
                if let Some(cat) = self.categories.get(self.selection) {
                    self.screen = Screen::Edit(CategoryForm::new_edit(cat));
                }
            }
            KeyCode::Char('d') => {
                if let Some(cat) = self.categories.get(self.selection) {
                    match categories::blocking_reason(conn, cat.id) {
                        Ok(Some(reason)) => self.set_status(reason),
                        Ok(None) => self.screen = Screen::ConfirmDelete,
                        Err(e) => self.set_status(format!("Error: {e}")),
                    }
                }
            }
            KeyCode::Char('q') | KeyCode::Esc => return CategoryAction::Close,
            _ => {}
        }
        CategoryAction::Continue
    }

    fn handle_form_key(
        &mut self,
        code: KeyCode,
        conn: &Connection,
        mode: FormMode,
    ) -> CategoryAction {
        let form = match &mut self.screen {
            Screen::Add(f) | Screen::Edit(f) => f,
            _ => return CategoryAction::Continue,
        };

        match code {
            KeyCode::Esc => {
                self.screen = Screen::List;
                return CategoryAction::Continue;
            }
            KeyCode::Tab | KeyCode::Down => {
                form.focused = (form.focused + 1) % form.fields.len();
            }
            KeyCode::BackTab | KeyCode::Up => {
                form.focused = if form.focused == 0 {
                    form.fields.len() - 1
                } else {
                    form.focused - 1
                };
            }
            KeyCode::Left => {
                let focused = form.focused;
                if let FieldKind::Selector { options, selected } =
                    &mut form.fields[form.focused].kind
                {
                    *selected = if *selected == 0 {
                        options.len() - 1
                    } else {
                        *selected - 1
                    };
                    form.fields[focused].value = options[*selected].clone();
                }
                if focused == CLASS_IDX {
                    form.class_chosen = true;
                }
            }
            KeyCode::Right => {
                let focused = form.focused;
                if let FieldKind::Selector { options, selected } =
                    &mut form.fields[form.focused].kind
                {
                    *selected = (*selected + 1) % options.len();
                    form.fields[focused].value = options[*selected].clone();
                }
                if focused == CLASS_IDX {
                    form.class_chosen = true;
                }
            }
            KeyCode::Char(c) => {
                if let FieldKind::Text = &form.fields[form.focused].kind {
                    form.fields[form.focused].value.push(c);
                }
            }
            KeyCode::Backspace => {
                if let FieldKind::Text = &form.fields[form.focused].kind {
                    form.fields[form.focused].value.pop();
                }
            }
            KeyCode::Enter => {
                let name = form.fields[NAME_IDX].value.trim().to_string();
                if name.is_empty() {
                    self.set_status("Name is required".into());
                    return CategoryAction::Continue;
                }
                let cat_type = form.fields[TYPE_IDX].value.clone();
                let class = match AccountClass::parse(&form.fields[CLASS_IDX].value) {
                    Some(class) => class,
                    None => {
                        self.set_status("Pick a class".into());
                        return CategoryAction::Continue;
                    }
                };
                // Absent means "derive it from the type", which is what an
                // untouched selector means on an add.
                let chosen_class = form.class_chosen.then_some(class);
                let tax_line = {
                    let v = form.fields[TAX_LINE_IDX].value.trim().to_string();
                    if v.is_empty() {
                        None
                    } else {
                        Some(v)
                    }
                };
                let form_line = {
                    let v = form.fields[FORM_LINE_IDX].value.trim().to_string();
                    if v.is_empty() {
                        None
                    } else {
                        Some(v)
                    }
                };

                match mode {
                    FormMode::Add => {
                        match categories::add_category(
                            conn,
                            &name,
                            &cat_type,
                            chosen_class,
                            tax_line.as_deref(),
                            form_line.as_deref(),
                        ) {
                            Ok(_) => {
                                self.reload(conn);
                                self.screen = Screen::List;
                                self.set_status(format!("Added category: {name}"));
                            }
                            Err(e) => {
                                self.set_status(e.to_string());
                            }
                        }
                    }
                    FormMode::Edit => {
                        if let Some(cat) = self.categories.get(self.selection) {
                            match categories::update_category(
                                conn,
                                cat.id,
                                &name,
                                &cat_type,
                                class,
                                tax_line.as_deref(),
                                form_line.as_deref(),
                            ) {
                                Ok(()) => {
                                    self.reload(conn);
                                    self.screen = Screen::List;
                                    self.set_status(format!("Updated category: {name}"));
                                }
                                Err(e) => {
                                    self.set_status(e.to_string());
                                }
                            }
                        }
                    }
                }
            }
            _ => {}
        }
        CategoryAction::Continue
    }

    fn handle_delete_key(&mut self, code: KeyCode, conn: &Connection) -> CategoryAction {
        match code {
            KeyCode::Char('y') => {
                if let Some(cat) = self.categories.get(self.selection) {
                    let name = cat.name.clone();
                    match categories::delete_category(conn, cat.id) {
                        Ok(()) => {
                            self.reload(conn);
                            self.screen = Screen::List;
                            self.set_status(format!("Deleted category: {name}"));
                        }
                        Err(e) => {
                            self.screen = Screen::List;
                            self.set_status(e.to_string());
                        }
                    }
                }
            }
            KeyCode::Char('n') | KeyCode::Esc => {
                self.screen = Screen::List;
            }
            _ => {}
        }
        CategoryAction::Continue
    }
}

enum FormMode {
    Add,
    Edit,
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max - 1).collect();
        format!("{truncated}\u{2026}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nigel_core::db::{get_connection, init_db, AccountClass};

    fn test_conn() -> (tempfile::TempDir, Connection) {
        let dir = tempfile::tempdir().unwrap();
        let conn = get_connection(&dir.path().join("t.db")).unwrap();
        init_db(&conn).unwrap();
        (dir, conn)
    }

    fn form_of(mgr: &CategoryManager) -> &CategoryForm {
        match &mgr.screen {
            Screen::Add(form) | Screen::Edit(form) => form,
            _ => panic!("not on a form"),
        }
    }

    #[test]
    fn the_add_form_offers_every_class_and_defaults_to_the_type() {
        let (_d, conn) = test_conn();
        let mut mgr = CategoryManager::new(&conn, "Hello.");
        mgr.handle_key(KeyCode::Char('a'), &conn);

        let field = &form_of(&mgr).fields[CLASS_IDX];
        assert_eq!(field.label, "Class");
        assert_eq!(field.value, "expense");
        match &field.kind {
            FieldKind::Selector { options, .. } => {
                assert_eq!(options.len(), AccountClass::ALL.len());
                assert!(options.iter().all(|o| AccountClass::parse(o).is_some()));
                // AC #6: the five words, and nothing borrowed from a ledger.
                assert!(!options.iter().any(|o| o == "debit" || o == "credit"));
            }
            FieldKind::Text => panic!("not a selector"),
        }
    }

    #[test]
    fn the_add_form_sends_no_class_until_the_operator_picks_one() {
        // Expense is what the control opens on; an income category whose class
        // was never touched must still be classified from its type.
        let (_d, conn) = test_conn();
        let mut mgr = CategoryManager::new(&conn, "Hello.");
        mgr.handle_key(KeyCode::Char('a'), &conn);
        for c in "Workshop Fees".chars() {
            mgr.handle_key(KeyCode::Char(c), &conn);
        }
        for _ in 0..TYPE_IDX {
            mgr.handle_key(KeyCode::Tab, &conn);
        }
        mgr.handle_key(KeyCode::Right, &conn); // expense -> income
        mgr.handle_key(KeyCode::Enter, &conn);

        let added = mgr
            .categories
            .iter()
            .find(|c| c.name == "Workshop Fees")
            .expect("the added category");
        assert_eq!(added.category_type, "income");
        assert_eq!(added.class, AccountClass::Revenue, "derived from the type");
    }

    #[test]
    fn a_class_the_operator_picked_on_the_add_form_is_the_one_saved() {
        let (_d, conn) = test_conn();
        let mut mgr = CategoryManager::new(&conn, "Hello.");
        mgr.handle_key(KeyCode::Char('a'), &conn);
        for c in "Member Draw".chars() {
            mgr.handle_key(KeyCode::Char(c), &conn);
        }
        for _ in 0..CLASS_IDX {
            mgr.handle_key(KeyCode::Tab, &conn);
        }
        mgr.handle_key(KeyCode::Left, &conn); // expense -> revenue
        mgr.handle_key(KeyCode::Left, &conn); // revenue -> equity
        mgr.handle_key(KeyCode::Enter, &conn);

        let added = mgr
            .categories
            .iter()
            .find(|c| c.name == "Member Draw")
            .expect("the added category");
        assert_eq!(added.category_type, "expense");
        assert_eq!(added.class, AccountClass::Equity, "not the type's expense");
    }

    #[test]
    fn an_edit_form_opens_on_the_categorys_own_class() {
        let (_d, conn) = test_conn();
        let mut mgr = CategoryManager::new(&conn, "Hello.");
        let draw = mgr
            .categories
            .iter()
            .position(|c| c.name == "Owner Draw / Distribution")
            .expect("the seeded distribution category");
        mgr.selection = draw;
        mgr.handle_key(KeyCode::Char('e'), &conn);

        assert_eq!(form_of(&mgr).fields[CLASS_IDX].value, "equity");
    }

    #[test]
    fn the_class_selector_cycles_and_saves_what_it_shows() {
        let (_d, conn) = test_conn();
        let mut mgr = CategoryManager::new(&conn, "Hello.");
        mgr.handle_key(KeyCode::Char('a'), &conn);
        // Name, then walk to the class field and pick the next class.
        for ch in "Member Draw".chars() {
            mgr.handle_key(KeyCode::Char(ch), &conn);
        }
        for _ in 0..CLASS_IDX {
            mgr.handle_key(KeyCode::Tab, &conn);
        }
        let before = form_of(&mgr).fields[CLASS_IDX].value.clone();
        mgr.handle_key(KeyCode::Right, &conn);
        let after = form_of(&mgr).fields[CLASS_IDX].value.clone();
        assert_ne!(before, after);

        mgr.handle_key(KeyCode::Enter, &conn);
        let saved = mgr
            .categories
            .iter()
            .find(|c| c.name == "Member Draw")
            .expect("saved");
        assert_eq!(saved.class.as_str(), after);
    }
}
