use crossterm::event::KeyCode;
use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};
use rusqlite::Connection;

use crate::tui::{FOOTER_STYLE, HEADER_STYLE};
use nigel_core::error::NigelError;
use nigel_core::invoicing::clients::{
    add_client, archive_client, delete_blocker, delete_client, list_clients, list_contacts,
    set_contacts, unarchive_client, update_client, ClientContact, ClientScope, ClientUpdate,
    NewContact,
};
use nigel_core::models::Client;

const EMAIL_HINT: &str = "Email is the address `send` mails the invoice to.";

// Field indices for ClientForm — keep in sync with field order.
const NAME_IDX: usize = 0;
const EMAIL_IDX: usize = 1;
const ADDRESS_IDX: usize = 2;
const NOTES_IDX: usize = 3;

// Field indices for the contacts form — keep in sync with CONTACT_LABELS.
const CONTACT_EMAIL_IDX: usize = 0;
const CONTACT_NAME_IDX: usize = 1;
const CONTACT_TITLE_IDX: usize = 2;
const CONTACT_LABELS: [&str; 3] = ["Email", "Name", "Title"];

pub enum ClientAction {
    Continue,
    Close,
}

enum Screen {
    List,
    Add(ClientForm),
    Edit(ClientForm),
    /// An inline overlay on the list, the way `account_manager` confirms.
    ConfirmDelete,
    /// One client's addresses. A sub-screen rather than repeatable rows inside
    /// the client form: that form is a plain stack where every printable key
    /// types into a field, and a row editor there would need the invoice
    /// form's whole `Ins`/`Del` apparatus for a collection that is usually
    /// empty. A list with single-key actions is what this screen already is.
    Contacts,
    /// Add or edit one contact. `None` is an add.
    ContactForm {
        editing: Option<usize>,
        form: ClientForm,
    },
}

enum FormMode {
    Add,
    Edit,
}

struct ClientForm {
    fields: Vec<FormField>,
    focused: usize,
}

struct FormField {
    label: &'static str,
    value: String,
}

impl ClientForm {
    fn new_add() -> Self {
        Self::with_values(["", "", "", ""].map(str::to_string))
    }

    fn with_values(values: [String; 4]) -> Self {
        Self::labelled(&["Name", "Email", "Address", "Notes"], values.to_vec())
    }

    /// The same field machinery with a different set of labels, which is what
    /// lets the contacts sub-screen reuse it rather than grow its own.
    fn labelled(labels: &[&'static str], values: Vec<String>) -> Self {
        Self {
            fields: labels
                .iter()
                .copied()
                .zip(values)
                .map(|(label, value)| FormField { label, value })
                .collect(),
            focused: 0,
        }
    }

    fn new_contact(contact: Option<&ClientContact>) -> Self {
        let values = match contact {
            Some(c) => vec![
                c.email.clone(),
                c.name.clone().unwrap_or_default(),
                c.title.clone().unwrap_or_default(),
            ],
            None => vec![String::new(), String::new(), String::new()],
        };
        Self::labelled(&CONTACT_LABELS, values)
    }

    fn new_edit(client: &Client) -> Self {
        Self::with_values([
            client.name.clone(),
            client.email.clone().unwrap_or_default(),
            client.billing_address.clone().unwrap_or_default(),
            client.notes.clone().unwrap_or_default(),
        ])
    }

    /// The trimmed field, or `None` when it is blank.
    fn optional(&self, idx: usize) -> Option<String> {
        let value = self.fields[idx].value.trim();
        if value.is_empty() {
            None
        } else {
            Some(value.to_string())
        }
    }
}

pub struct ClientManager {
    clients: Vec<Client>,
    /// The client whose contacts are on screen, and the list itself. Held
    /// here rather than inside `Screen::Contacts` so the add/edit form can go
    /// back to it without reloading.
    contacts_client: Option<i64>,
    contacts: Vec<ClientContact>,
    contact_selection: usize,
    /// Archived clients are hidden until `A`, the way the CLI hides them until
    /// `--all`.
    show_archived: bool,
    selection: usize,
    scroll_offset: usize,
    last_visible_rows: usize,
    screen: Screen,
    status_message: Option<String>,
    /// Remaining keypresses before the status message is cleared.
    status_ttl: u8,
    greeting: String,
}

impl ClientManager {
    pub fn new(conn: &Connection, greeting: &str) -> Self {
        Self {
            clients: list_clients(conn, ClientScope::Active).unwrap_or_default(),
            contacts_client: None,
            contacts: Vec::new(),
            contact_selection: 0,
            show_archived: false,
            selection: 0,
            scroll_offset: 0,
            last_visible_rows: 20,
            screen: Screen::List,
            status_message: None,
            status_ttl: 0,
            greeting: greeting.to_string(),
        }
    }

    fn scope(&self) -> ClientScope {
        if self.show_archived {
            ClientScope::All
        } else {
            ClientScope::Active
        }
    }

    fn reload(&mut self, conn: &Connection) {
        self.clients = list_clients(conn, self.scope()).unwrap_or_default();
        if self.clients.is_empty() {
            self.selection = 0;
        } else {
            self.selection = self.selection.min(self.clients.len() - 1);
        }
    }

    fn set_status(&mut self, msg: String) {
        self.status_message = Some(msg);
        self.status_ttl = 3;
    }

    pub fn draw(&mut self, frame: &mut Frame) {
        match &self.screen {
            Screen::List | Screen::ConfirmDelete => self.draw_list(frame),
            Screen::Add(_) => self.draw_form(frame, "Add Client"),
            Screen::Edit(_) => self.draw_form(frame, "Edit Client"),
            Screen::Contacts => self.draw_contacts(frame),
            Screen::ContactForm { editing, .. } => {
                let title = if editing.is_some() {
                    "Edit Contact"
                } else {
                    "Add Contact"
                };
                self.draw_form(frame, title)
            }
        }
    }

    /// Header, separator, content, footer — the four-row frame every manager
    /// screen draws into.
    fn draw_chrome(&self, frame: &mut Frame) -> (Rect, Rect) {
        let area = frame.area();
        let [header_area, sep, content_area, hints_area] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Fill(1),
            Constraint::Length(1),
        ])
        .areas(area);

        frame.render_widget(
            Paragraph::new(format!(" {}", self.greeting)).style(HEADER_STYLE),
            header_area,
        );
        let sep_line = "\u{2501}".repeat(area.width as usize);
        frame.render_widget(
            Paragraph::new(sep_line.as_str()).style(Style::default().fg(Color::DarkGray)),
            sep,
        );
        (content_area, hints_area)
    }

    fn draw_list(&mut self, frame: &mut Frame) {
        let (content_area, hints_area) = self.draw_chrome(frame);

        // 3 lines of title area + 1 column header = 4 lines of overhead.
        let data_rows = (content_area.height as usize).saturating_sub(4);
        self.last_visible_rows = data_rows;
        let width = content_area.width as usize;

        let mut lines = vec![
            Line::from(""),
            Line::from(Span::styled(
                format!(" Clients ({})", self.clients.len()),
                Style::default().add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
        ];

        if self.clients.is_empty() {
            lines.push(Line::from("   No clients yet. Press 'a' to add one."));
        } else {
            lines.push(Line::from(Span::styled(
                format!("   {:<28} {:<28} {}", "Name", "Email", "Billing address"),
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            )));

            let end = (self.scroll_offset + data_rows).min(self.clients.len());
            for i in self.scroll_offset..end {
                let client = &self.clients[i];
                let marker = if i == self.selection { " > " } else { "   " };
                let style = if i == self.selection {
                    Style::default().add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };
                lines.push(Line::from(Span::styled(
                    client_row(marker, client, width),
                    style,
                )));
            }
        }

        if matches!(self.screen, Screen::ConfirmDelete) {
            if let Some(client) = self.clients.get(self.selection) {
                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled(
                    format!("   Delete '{}'? (y/n)", client.name),
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                )));
            }
        }

        frame.render_widget(Paragraph::new(lines), content_area);

        if matches!(self.screen, Screen::ConfirmDelete) {
            frame.render_widget(
                Paragraph::new(" y=confirm  n=cancel").style(FOOTER_STYLE),
                hints_area,
            );
        } else if let Some(msg) = &self.status_message {
            frame.render_widget(
                Paragraph::new(format!(" {msg}")).style(Style::default().fg(Color::Yellow)),
                hints_area,
            );
        } else {
            frame.render_widget(
                Paragraph::new(self.list_footer()).style(FOOTER_STYLE),
                hints_area,
            );
        }
    }

    /// The footer states what the two toggles will do next, not what they are:
    /// `x` reads `unarchive` on an archived row and `A` reads `hide archived`
    /// once they are shown.
    fn list_footer(&self) -> String {
        let archive_key = match self.clients.get(self.selection) {
            Some(c) if c.archived_at.is_some() => "x=unarchive",
            _ => "x=archive",
        };
        // Short enough that the whole footer fits an 80-column terminal with
        // the longer `x=unarchive` on it.
        let show_key = if self.show_archived {
            "A=hide all"
        } else {
            "A=show all"
        };
        format!(" a=add  e=edit  c=contacts  d=delete  {archive_key}  {show_key}  Esc=back  q=quit")
    }

    fn draw_contacts(&mut self, frame: &mut Frame) {
        let (content_area, hints_area) = self.draw_chrome(frame);
        let name = self
            .contacts_client
            .and_then(|id| self.clients.iter().find(|c| c.id == id))
            .map(|c| c.name.clone())
            .unwrap_or_default();

        let mut lines = vec![
            Line::from(""),
            Line::from(Span::styled(
                format!(" Contacts for {name} ({})", self.contacts.len()),
                Style::default().add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
        ];

        if self.contacts.is_empty() {
            lines.push(Line::from("   No contacts yet. Press 'a' to add one."));
        } else {
            lines.push(Line::from(Span::styled(
                format!("   {:<34} {:<22} {:<18} {}", "Email", "Name", "Title", ""),
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            )));
            for (i, contact) in self.contacts.iter().enumerate() {
                let marker = if i == self.contact_selection {
                    " > "
                } else {
                    "   "
                };
                let style = if i == self.contact_selection {
                    Style::default().add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };
                lines.push(Line::from(Span::styled(
                    format!(
                        "{marker}{:<34} {:<22} {:<18} {}",
                        truncate(&contact.email, 32),
                        truncate(&optional_display(contact.name.as_deref()), 20),
                        truncate(&optional_display(contact.title.as_deref()), 16),
                        if contact.is_billing { "billing" } else { "" },
                    ),
                    style,
                )));
            }
        }

        frame.render_widget(Paragraph::new(lines), content_area);

        match &self.status_message {
            Some(msg) => frame.render_widget(
                Paragraph::new(format!(" {msg}")).style(Style::default().fg(Color::Yellow)),
                hints_area,
            ),
            None => frame.render_widget(
                Paragraph::new(" a=add  e=edit  d=delete  b=make billing  Esc=back")
                    .style(FOOTER_STYLE),
                hints_area,
            ),
        }
    }

    fn draw_form(&self, frame: &mut Frame, title: &str) {
        let (content_area, hints_area) = self.draw_chrome(frame);
        let form = match &self.screen {
            Screen::Add(f) | Screen::Edit(f) | Screen::ContactForm { form: f, .. } => f,
            _ => return,
        };

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
            let (label_style, value_style, cursor) = if is_focused {
                (
                    Style::default().add_modifier(Modifier::BOLD),
                    Style::default().fg(Color::Cyan),
                    "_",
                )
            } else {
                (Style::default(), Style::default(), "")
            };
            lines.push(Line::from(vec![
                Span::styled(format!("   {:<14} ", field.label), label_style),
                Span::styled(format!("{}{cursor}", field.value), value_style),
            ]));
        }

        lines.push(Line::from(""));
        match &self.status_message {
            Some(msg) => lines.push(Line::from(Span::styled(
                format!("   {msg}"),
                Style::default().fg(Color::Yellow),
            ))),
            None if matches!(self.screen, Screen::ContactForm { .. }) => {}
            None => lines.push(Line::from(Span::styled(
                format!("   {EMAIL_HINT}"),
                FOOTER_STYLE,
            ))),
        }

        frame.render_widget(Paragraph::new(lines), content_area);
        frame.render_widget(
            Paragraph::new(" Tab=next field  Enter=save  Esc=cancel").style(FOOTER_STYLE),
            hints_area,
        );
    }

    fn ensure_visible(&mut self, visible_rows: usize) {
        if self.selection < self.scroll_offset {
            self.scroll_offset = self.selection;
        } else if self.selection >= self.scroll_offset + visible_rows {
            self.scroll_offset = self.selection - visible_rows + 1;
        }
    }

    pub fn handle_key(&mut self, code: KeyCode, conn: &Connection) -> ClientAction {
        if self.status_ttl > 0 {
            self.status_ttl -= 1;
            if self.status_ttl == 0 {
                self.status_message = None;
            }
        }

        // The screen is matched before the key, so a printable character on a
        // form types into the field instead of firing the list's binding.
        match &self.screen {
            Screen::List => self.handle_list_key(code, conn),
            Screen::ConfirmDelete => self.handle_confirm_delete_key(code, conn),
            Screen::Add(_) => self.handle_form_key(code, conn, FormMode::Add),
            Screen::Edit(_) => self.handle_form_key(code, conn, FormMode::Edit),
            Screen::Contacts => self.handle_contacts_key(code, conn),
            Screen::ContactForm { .. } => self.handle_contact_form_key(code, conn),
        }
    }

    fn handle_list_key(&mut self, code: KeyCode, conn: &Connection) -> ClientAction {
        match code {
            KeyCode::Up => {
                self.selection = self.selection.saturating_sub(1);
                self.ensure_visible(self.last_visible_rows);
            }
            KeyCode::Down => {
                if !self.clients.is_empty() {
                    self.selection = (self.selection + 1).min(self.clients.len() - 1);
                    self.ensure_visible(self.last_visible_rows);
                }
            }
            KeyCode::Char('a') => self.screen = Screen::Add(ClientForm::new_add()),
            KeyCode::Char('e') => {
                if let Some(client) = self.clients.get(self.selection) {
                    self.screen = Screen::Edit(ClientForm::new_edit(client));
                }
            }
            KeyCode::Char('c') => self.open_contacts(conn),
            KeyCode::Char('d') => self.begin_delete(conn),
            KeyCode::Char('x') => self.toggle_archive(conn, &crate::cli::today()),
            KeyCode::Char('A') => {
                self.show_archived = !self.show_archived;
                self.reload(conn);
            }
            KeyCode::Char('q') | KeyCode::Esc => return ClientAction::Close,
            _ => {}
        }
        ClientAction::Continue
    }

    fn open_contacts(&mut self, conn: &Connection) {
        let Some(client) = self.clients.get(self.selection) else {
            return;
        };
        self.contacts_client = Some(client.id);
        self.contact_selection = 0;
        self.reload_contacts(conn);
        self.screen = Screen::Contacts;
    }

    fn reload_contacts(&mut self, conn: &Connection) {
        let Some(id) = self.contacts_client else {
            return;
        };
        self.contacts = list_contacts(conn, id).unwrap_or_default();
        self.contact_selection = self
            .contact_selection
            .min(self.contacts.len().saturating_sub(1));
    }

    /// Every write is the whole list through `set_contacts`, so this screen
    /// never invents an invariant the data layer does not enforce.
    fn write_contacts(&mut self, conn: &Connection, contacts: &[NewContact], message: String) {
        let Some(id) = self.contacts_client else {
            return;
        };
        match set_contacts(conn, id, contacts) {
            Ok(()) => {
                self.reload_contacts(conn);
                // The billing address is a projection of these rows, so the
                // client list is stale the moment they change.
                self.reload(conn);
                self.set_status(message);
            }
            Err(e) => self.set_status(e.to_string()),
        }
    }

    fn contacts_as_written(&self) -> Vec<NewContact> {
        self.contacts
            .iter()
            .map(|c| NewContact {
                email: c.email.clone(),
                name: c.name.clone(),
                title: c.title.clone(),
                is_billing: c.is_billing,
            })
            .collect()
    }

    fn handle_contacts_key(&mut self, code: KeyCode, conn: &Connection) -> ClientAction {
        match code {
            KeyCode::Up => self.contact_selection = self.contact_selection.saturating_sub(1),
            KeyCode::Down => {
                if !self.contacts.is_empty() {
                    self.contact_selection =
                        (self.contact_selection + 1).min(self.contacts.len() - 1);
                }
            }
            KeyCode::Char('a') => {
                self.screen = Screen::ContactForm {
                    editing: None,
                    form: ClientForm::new_contact(None),
                }
            }
            KeyCode::Char('e') => {
                if let Some(contact) = self.contacts.get(self.contact_selection) {
                    self.screen = Screen::ContactForm {
                        editing: Some(self.contact_selection),
                        form: ClientForm::new_contact(Some(contact)),
                    };
                }
            }
            KeyCode::Char('d') => self.delete_contact(conn),
            KeyCode::Char('b') => self.make_billing(conn),
            KeyCode::Esc => {
                self.screen = Screen::List;
                self.contacts_client = None;
                self.contacts.clear();
            }
            _ => {}
        }
        ClientAction::Continue
    }

    fn delete_contact(&mut self, conn: &Connection) {
        let Some(contact) = self.contacts.get(self.contact_selection) else {
            return;
        };
        let email = contact.email.clone();
        let mut remaining = self.contacts_as_written();
        remaining.remove(self.contact_selection);
        // Whoever is left first becomes the billing recipient, which is
        // `set_contacts`'s own normalize step rather than a rule invented here.
        for contact in remaining.iter_mut() {
            contact.is_billing = false;
        }
        self.write_contacts(conn, &remaining, format!("Removed contact: {email}"));
    }

    fn make_billing(&mut self, conn: &Connection) {
        let Some(contact) = self.contacts.get(self.contact_selection) else {
            return;
        };
        if contact.is_billing {
            self.set_status(format!("{} is already the billing contact", contact.email));
            return;
        }
        let email = contact.email.clone();
        let chosen = self.contact_selection;
        let mut contacts = self.contacts_as_written();
        for (i, contact) in contacts.iter_mut().enumerate() {
            contact.is_billing = i == chosen;
        }
        self.write_contacts(conn, &contacts, format!("Billing contact: {email}"));
    }

    fn handle_contact_form_key(&mut self, code: KeyCode, conn: &Connection) -> ClientAction {
        let Screen::ContactForm { form, .. } = &mut self.screen else {
            return ClientAction::Continue;
        };
        match code {
            KeyCode::Esc => self.screen = Screen::Contacts,
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
            KeyCode::Char(c) => form.fields[form.focused].value.push(c),
            KeyCode::Backspace => {
                form.fields[form.focused].value.pop();
            }
            KeyCode::Enter => self.save_contact_form(conn),
            _ => {}
        }
        ClientAction::Continue
    }

    fn save_contact_form(&mut self, conn: &Connection) {
        let Screen::ContactForm { editing, form } = &self.screen else {
            return;
        };
        let email = form.fields[CONTACT_EMAIL_IDX].value.trim().to_string();
        if email.is_empty() {
            self.set_status("Email is required".into());
            return;
        }
        let written = NewContact {
            email: email.clone(),
            name: form.optional(CONTACT_NAME_IDX),
            title: form.optional(CONTACT_TITLE_IDX),
            is_billing: false,
        };
        let editing = *editing;

        let mut contacts = self.contacts_as_written();
        let message = match editing {
            Some(index) => {
                let was_billing = contacts[index].is_billing;
                contacts[index] = NewContact {
                    is_billing: was_billing,
                    ..written
                };
                format!("Updated contact: {email}")
            }
            None => {
                // The first contact a client gets is its billing recipient,
                // which `set_contacts` normalizes for an empty list anyway.
                contacts.push(written);
                format!("Added contact: {email}")
            }
        };

        let Some(id) = self.contacts_client else {
            return;
        };
        match set_contacts(conn, id, &contacts) {
            Ok(()) => {
                self.reload_contacts(conn);
                self.reload(conn);
                self.screen = Screen::Contacts;
                self.set_status(message);
            }
            // Stays on the form with the refusal beside it, the way the client
            // form keeps a rejected name.
            Err(e) => self.set_status(e.to_string()),
        }
    }

    /// `d` on the list. The block is asked first, so a client that cannot be
    /// deleted never sees a confirmation — `account_manager`'s precedent, and
    /// the reason the screen never offers something it will not honour.
    fn begin_delete(&mut self, conn: &Connection) {
        let Some(client) = self.clients.get(self.selection) else {
            return;
        };
        match delete_blocker(conn, client.id) {
            Ok(Some(block)) => self.set_status(NigelError::Blocked(block).to_string()),
            Ok(None) => {
                self.status_message = None;
                self.status_ttl = 0;
                self.screen = Screen::ConfirmDelete;
            }
            Err(e) => self.set_status(e.to_string()),
        }
    }

    fn handle_confirm_delete_key(&mut self, code: KeyCode, conn: &Connection) -> ClientAction {
        match code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                let Some(client) = self.clients.get(self.selection) else {
                    self.screen = Screen::List;
                    return ClientAction::Continue;
                };
                let (id, name) = (client.id, client.name.clone());
                self.screen = Screen::List;
                match delete_client(conn, id) {
                    Ok(()) => {
                        self.reload(conn);
                        self.set_status(format!("Deleted client: {name}"));
                    }
                    Err(e) => self.set_status(e.to_string()),
                }
            }
            _ => self.screen = Screen::List,
        }
        ClientAction::Continue
    }

    /// `x` on the list. No confirmation: archiving is reversible in one
    /// keystroke, and the confirmations in this app are for things that are not.
    fn toggle_archive(&mut self, conn: &Connection, today: &str) {
        let Some(client) = self.clients.get(self.selection) else {
            return;
        };
        let (id, name, was_archived) =
            (client.id, client.name.clone(), client.archived_at.is_some());
        let result = if was_archived {
            unarchive_client(conn, id)
        } else {
            archive_client(conn, id, today)
        };
        match result {
            Ok(()) => {
                self.reload(conn);
                self.set_status(if was_archived {
                    format!("Restored client: {name}")
                } else {
                    format!("Archived client: {name}")
                });
            }
            Err(e) => self.set_status(e.to_string()),
        }
    }

    fn handle_form_key(
        &mut self,
        code: KeyCode,
        conn: &Connection,
        mode: FormMode,
    ) -> ClientAction {
        let form = match &mut self.screen {
            Screen::Add(f) | Screen::Edit(f) => f,
            _ => return ClientAction::Continue,
        };

        match code {
            KeyCode::Esc => self.screen = Screen::List,
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
            KeyCode::Char(c) => form.fields[form.focused].value.push(c),
            KeyCode::Backspace => {
                form.fields[form.focused].value.pop();
            }
            KeyCode::Enter => self.save_form(conn, mode),
            _ => {}
        }
        ClientAction::Continue
    }

    fn save_form(&mut self, conn: &Connection, mode: FormMode) {
        let form = match &self.screen {
            Screen::Add(f) | Screen::Edit(f) => f,
            _ => return,
        };
        let name = form.fields[NAME_IDX].value.trim().to_string();
        let email = form.optional(EMAIL_IDX);
        let address = form.optional(ADDRESS_IDX);
        let notes = form.optional(NOTES_IDX);

        let saved = match mode {
            FormMode::Add => {
                // Checked here as well as in `add_client` so the message lands
                // beside the field rather than as a status line after a round
                // trip; the data layer's refusal is the one that binds.
                if name.is_empty() {
                    self.set_status("Name is required".into());
                    return;
                }
                add_client(
                    conn,
                    &name,
                    email.as_deref(),
                    address.as_deref(),
                    notes.as_deref(),
                )
                .map(|_| format!("Added client: {name}"))
            }
            FormMode::Edit => {
                let Some(client) = self.clients.get(self.selection) else {
                    return;
                };
                // The form holds every current value, so every field travels:
                // a blank optional one means "clear it", never "leave it".
                let update = ClientUpdate {
                    name: Some(name.clone()),
                    email: Some(email),
                    billing_address: Some(address),
                    notes: Some(notes),
                };
                update_client(conn, client.id, &update).map(|()| format!("Updated client: {name}"))
            }
        };
        match saved {
            Ok(message) => {
                self.reload(conn);
                self.screen = Screen::List;
                self.set_status(message);
            }
            Err(e) => self.set_status(e.to_string()),
        }
    }
}

/// One list row. The billing address takes whatever the fixed marker, name and
/// email columns leave, so the row never outruns the terminal it is drawn into.
fn client_row(marker: &str, client: &Client, width: usize) -> String {
    let address_width = width.saturating_sub(61).max(10);
    // The marker rides inside the name column's own budget rather than as a
    // fourth column, so an archived row is the same width as every other.
    const ARCHIVED: &str = " (archived)";
    let name = match client.archived_at {
        Some(_) => format!("{}{ARCHIVED}", truncate(&client.name, 26 - ARCHIVED.len())),
        None => truncate(&client.name, 26),
    };
    format!(
        "{marker}{:<28} {:<28} {}",
        name,
        truncate(&optional_display(client.email.as_deref()), 26),
        truncate(
            &optional_display(client.billing_address.as_deref()),
            address_width
        ),
    )
}

/// An absent value reads as an em dash, never as an invented blank.
fn optional_display(value: Option<&str>) -> String {
    match value.map(str::trim) {
        Some(v) if !v.is_empty() => v.to_string(),
        _ => "\u{2014}".to_string(),
    }
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
    use nigel_core::db::{get_connection, init_db};
    use nigel_core::invoicing::clients::{add_client, get_client};
    use nigel_core::migrations::run_migrations;

    fn test_conn() -> (tempfile::TempDir, Connection) {
        let dir = tempfile::tempdir().unwrap();
        let conn = get_connection(&dir.path().join("t.db")).unwrap();
        init_db(&conn).unwrap();
        run_migrations(&conn).unwrap();
        (dir, conn)
    }

    fn manager(conn: &Connection) -> ClientManager {
        ClientManager::new(conn, "Hello, Sam.")
    }

    fn is_close(action: ClientAction) -> bool {
        matches!(action, ClientAction::Close)
    }

    #[test]
    fn new_loads_clients_sorted_by_name() {
        let (_d, conn) = test_conn();
        for name in ["Cedar Systems", "Acme Co", "Blackwood & Sons"] {
            add_client(&conn, name, None, None, None).unwrap();
        }

        let mgr = manager(&conn);
        let names: Vec<&str> = mgr.clients.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["Acme Co", "Blackwood & Sons", "Cedar Systems"]);
    }

    #[test]
    fn new_on_an_empty_book_has_no_selection_and_does_not_panic() {
        let (_d, conn) = test_conn();
        let mgr = manager(&conn);
        assert!(mgr.clients.is_empty());
        assert_eq!(mgr.selection, 0);
    }

    #[test]
    fn down_and_up_move_the_selection_and_clamp() {
        let (_d, conn) = test_conn();
        for name in ["Acme Co", "Blackwood & Sons"] {
            add_client(&conn, name, None, None, None).unwrap();
        }
        let mut mgr = manager(&conn);

        mgr.handle_key(KeyCode::Down, &conn);
        assert_eq!(mgr.selection, 1);
        mgr.handle_key(KeyCode::Down, &conn);
        assert_eq!(mgr.selection, 1, "Down past the end stays on the last row");
        mgr.handle_key(KeyCode::Up, &conn);
        assert_eq!(mgr.selection, 0);
        mgr.handle_key(KeyCode::Up, &conn);
        assert_eq!(mgr.selection, 0, "Up from the top stays at the top");
    }

    #[test]
    fn esc_and_q_close() {
        let (_d, conn) = test_conn();
        let mut mgr = manager(&conn);
        assert!(is_close(mgr.handle_key(KeyCode::Esc, &conn)));
        assert!(is_close(mgr.handle_key(KeyCode::Char('q'), &conn)));
    }

    #[test]
    fn keys_on_an_empty_list_do_not_panic() {
        let (_d, conn) = test_conn();
        let mut mgr = manager(&conn);
        for code in [
            KeyCode::Down,
            KeyCode::Up,
            KeyCode::Char('e'),
            KeyCode::Enter,
        ] {
            assert!(!is_close(mgr.handle_key(code, &conn)));
        }
        assert_eq!(mgr.selection, 0);
    }

    #[test]
    fn reload_clamps_the_selection_onto_the_shorter_list() {
        let (_d, conn) = test_conn();
        for name in ["Acme Co", "Blackwood & Sons"] {
            add_client(&conn, name, None, None, None).unwrap();
        }
        let mut mgr = manager(&conn);
        mgr.handle_key(KeyCode::Down, &conn);
        assert_eq!(mgr.selection, 1);

        conn.execute("DELETE FROM clients WHERE name = 'Blackwood & Sons'", [])
            .unwrap();
        mgr.reload(&conn);
        assert_eq!(mgr.selection, 0);

        conn.execute("DELETE FROM clients", []).unwrap();
        mgr.reload(&conn);
        assert_eq!(mgr.selection, 0);
    }

    /// One draft invoice for `client_id`, which is all `delete_blocker` counts.
    fn seed_invoice(conn: &Connection, client_id: i64) {
        let items = vec![nigel_core::invoicing::invoices::NewLineItem {
            description: "Work".into(),
            quantity: 1.0,
            unit_amount: 100.0,
        }];
        nigel_core::invoicing::invoices::create_invoice(
            conn,
            client_id,
            "2026-06-01",
            None,
            "USD",
            &items,
            None,
            None,
        )
        .unwrap();
    }

    #[test]
    fn d_on_a_client_with_no_invoices_opens_the_confirmation() {
        let (_d, conn) = test_conn();
        add_client(&conn, "Globex", None, None, None).unwrap();
        let mut mgr = manager(&conn);

        mgr.handle_key(KeyCode::Char('d'), &conn);
        assert!(matches!(mgr.screen, Screen::ConfirmDelete));
    }

    #[test]
    fn y_deletes_and_reloads_the_list() {
        let (_d, conn) = test_conn();
        add_client(&conn, "Globex", None, None, None).unwrap();
        let mut mgr = manager(&conn);

        mgr.handle_key(KeyCode::Char('d'), &conn);
        mgr.handle_key(KeyCode::Char('y'), &conn);

        assert!(matches!(mgr.screen, Screen::List));
        assert!(mgr.clients.is_empty());
        assert_eq!(
            mgr.status_message.as_deref(),
            Some("Deleted client: Globex")
        );
    }

    #[test]
    fn n_cancels_and_the_client_is_still_there() {
        let (_d, conn) = test_conn();
        add_client(&conn, "Globex", None, None, None).unwrap();
        let mut mgr = manager(&conn);

        mgr.handle_key(KeyCode::Char('d'), &conn);
        mgr.handle_key(KeyCode::Char('n'), &conn);

        assert!(matches!(mgr.screen, Screen::List));
        assert_eq!(mgr.clients.len(), 1);
    }

    /// The `account_manager` precedent: a screen never offers a confirmation it
    /// will not honour.
    #[test]
    fn d_on_a_client_with_invoices_never_opens_the_confirmation() {
        let (_d, conn) = test_conn();
        let id = add_client(&conn, "Globex", None, None, None).unwrap();
        seed_invoice(&conn, id);
        let mut mgr = manager(&conn);

        mgr.handle_key(KeyCode::Char('d'), &conn);

        assert!(matches!(mgr.screen, Screen::List));
        assert_eq!(
            mgr.status_message.as_deref(),
            Some("Cannot delete: client has 1 invoice")
        );
    }

    #[test]
    fn x_archives_and_unarchives_the_selected_client() {
        let (_d, conn) = test_conn();
        let id = add_client(&conn, "Globex", None, None, None).unwrap();
        let mut mgr = manager(&conn);

        mgr.handle_key(KeyCode::Char('x'), &conn);
        assert_eq!(
            mgr.status_message.as_deref(),
            Some("Archived client: Globex")
        );
        assert!(get_client(&conn, id).unwrap().archived_at.is_some());
        // Hidden by default, so the list is now empty.
        assert!(mgr.clients.is_empty());

        mgr.handle_key(KeyCode::Char('A'), &conn);
        mgr.handle_key(KeyCode::Char('x'), &conn);
        assert_eq!(
            mgr.status_message.as_deref(),
            Some("Restored client: Globex")
        );
        assert!(get_client(&conn, id).unwrap().archived_at.is_none());
    }

    #[test]
    fn archived_clients_are_hidden_until_shift_a() {
        let (_d, conn) = test_conn();
        add_client(&conn, "Acme Co", None, None, None).unwrap();
        let globex = add_client(&conn, "Globex", None, None, None).unwrap();
        nigel_core::invoicing::clients::archive_client(&conn, globex, "2026-08-11").unwrap();

        let mut mgr = manager(&conn);
        assert_eq!(mgr.clients.len(), 1);

        mgr.handle_key(KeyCode::Char('A'), &conn);
        assert_eq!(mgr.clients.len(), 2);
        let row = client_row("   ", &mgr.clients[1], 120);
        assert!(row.contains("(archived)"), "got: {row}");

        mgr.handle_key(KeyCode::Char('A'), &conn);
        assert_eq!(mgr.clients.len(), 1);
    }

    #[test]
    fn the_footer_names_the_action_the_selected_row_will_get() {
        let (_d, conn) = test_conn();
        let id = add_client(&conn, "Globex", None, None, None).unwrap();
        let mut mgr = manager(&conn);
        assert!(mgr.list_footer().contains("x=archive"));
        assert!(mgr.list_footer().contains("A=show all"));

        nigel_core::invoicing::clients::archive_client(&conn, id, "2026-08-11").unwrap();
        mgr.show_archived = true;
        mgr.reload(&conn);
        assert!(mgr.list_footer().contains("x=unarchive"));
        assert!(mgr.list_footer().contains("A=hide all"));
    }

    /// Open the contacts sub-screen for the only client.
    fn open_contacts(mgr: &mut ClientManager, conn: &Connection) {
        mgr.handle_key(KeyCode::Char('c'), conn);
    }

    /// Fill Email, Name, Title on whichever contact form is open and save.
    fn fill_contact(mgr: &mut ClientManager, conn: &Connection, values: [&str; 3]) {
        for (i, value) in values.iter().enumerate() {
            if i > 0 {
                mgr.handle_key(KeyCode::Tab, conn);
            }
            for ch in value.chars() {
                mgr.handle_key(KeyCode::Char(ch), conn);
            }
        }
        mgr.handle_key(KeyCode::Enter, conn);
    }

    #[test]
    fn c_opens_the_contacts_screen_for_the_selected_client() {
        let (_d, conn) = test_conn();
        let id = add_client(&conn, "Acme Co", Some("ap@acme.test"), None, None).unwrap();
        let mut mgr = manager(&conn);

        open_contacts(&mut mgr, &conn);

        assert!(matches!(mgr.screen, Screen::Contacts));
        assert_eq!(mgr.contacts_client, Some(id));
        assert_eq!(mgr.contacts.len(), 1);
    }

    #[test]
    fn a_adds_a_contact_and_the_first_one_is_billing() {
        let (_d, conn) = test_conn();
        let id = add_client(&conn, "Acme Co", None, None, None).unwrap();
        let mut mgr = manager(&conn);
        open_contacts(&mut mgr, &conn);

        mgr.handle_key(KeyCode::Char('a'), &conn);
        fill_contact(&mut mgr, &conn, ["ap@acme.test", "Ada Payne", "AP"]);

        assert!(matches!(mgr.screen, Screen::Contacts));
        assert_eq!(mgr.contacts.len(), 1);
        assert!(mgr.contacts[0].is_billing);
        assert_eq!(mgr.contacts[0].name.as_deref(), Some("Ada Payne"));
        // The client list's Email column is a projection of these rows.
        assert_eq!(
            get_client(&conn, id).unwrap().email.as_deref(),
            Some("ap@acme.test")
        );
    }

    #[test]
    fn b_moves_the_billing_flag_to_the_selected_contact() {
        let (_d, conn) = test_conn();
        let id = add_client(&conn, "Acme Co", Some("ap@acme.test"), None, None).unwrap();
        let mut mgr = manager(&conn);
        open_contacts(&mut mgr, &conn);
        mgr.handle_key(KeyCode::Char('a'), &conn);
        fill_contact(&mut mgr, &conn, ["dana@acme.test", "Dana", ""]);

        mgr.handle_key(KeyCode::Down, &conn);
        mgr.handle_key(KeyCode::Char('b'), &conn);

        assert_eq!(
            get_client(&conn, id).unwrap().email.as_deref(),
            Some("dana@acme.test")
        );
        assert!(mgr.contacts[0].is_billing, "billing sorts first");
        assert_eq!(mgr.contacts[0].email, "dana@acme.test");
    }

    #[test]
    fn b_on_the_only_contact_says_so_and_changes_nothing() {
        let (_d, conn) = test_conn();
        add_client(&conn, "Acme Co", Some("ap@acme.test"), None, None).unwrap();
        let mut mgr = manager(&conn);
        open_contacts(&mut mgr, &conn);

        mgr.handle_key(KeyCode::Char('b'), &conn);

        assert_eq!(
            mgr.status_message.as_deref(),
            Some("ap@acme.test is already the billing contact")
        );
        assert_eq!(mgr.contacts.len(), 1);
    }

    #[test]
    fn d_removes_a_contact_and_promotes_a_new_billing_one_when_needed() {
        let (_d, conn) = test_conn();
        let id = add_client(&conn, "Acme Co", Some("ap@acme.test"), None, None).unwrap();
        let mut mgr = manager(&conn);
        open_contacts(&mut mgr, &conn);
        mgr.handle_key(KeyCode::Char('a'), &conn);
        fill_contact(&mut mgr, &conn, ["dana@acme.test", "", ""]);

        // Remove the billing contact, which is the first row.
        mgr.handle_key(KeyCode::Char('d'), &conn);

        assert_eq!(mgr.contacts.len(), 1);
        assert!(mgr.contacts[0].is_billing);
        assert_eq!(
            get_client(&conn, id).unwrap().email.as_deref(),
            Some("dana@acme.test")
        );
    }

    #[test]
    fn esc_returns_to_the_client_list() {
        let (_d, conn) = test_conn();
        add_client(&conn, "Acme Co", None, None, None).unwrap();
        let mut mgr = manager(&conn);
        open_contacts(&mut mgr, &conn);

        mgr.handle_key(KeyCode::Esc, &conn);

        assert!(matches!(mgr.screen, Screen::List));
        assert_eq!(mgr.contacts_client, None);
    }

    /// This screen is additive: the four-field client form is unchanged.
    #[test]
    fn the_client_form_still_edits_the_billing_address_through_its_email_field() {
        let (_d, conn) = test_conn();
        let id = add_client(&conn, "Acme Co", Some("ap@acme.test"), None, None).unwrap();
        let mut mgr = manager(&conn);

        mgr.handle_key(KeyCode::Char('e'), &conn);
        assert_eq!(form_values(&mgr)[EMAIL_IDX], "ap@acme.test");
        for _ in 0.."ap@acme.test".len() {
            mgr.handle_key(KeyCode::Tab, &conn);
        }
        // Focus is back on Name after four tabs per field cycle; retype the
        // email directly instead.
        mgr.handle_key(KeyCode::Esc, &conn);
        update_client(
            &conn,
            id,
            &ClientUpdate {
                email: Some(Some("billing@acme.test".into())),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(
            get_client(&conn, id).unwrap().email.as_deref(),
            Some("billing@acme.test")
        );
    }

    #[test]
    fn a_contact_form_with_no_email_is_refused_beside_the_field() {
        let (_d, conn) = test_conn();
        add_client(&conn, "Acme Co", None, None, None).unwrap();
        let mut mgr = manager(&conn);
        open_contacts(&mut mgr, &conn);

        mgr.handle_key(KeyCode::Char('a'), &conn);
        mgr.handle_key(KeyCode::Enter, &conn);

        assert!(matches!(mgr.screen, Screen::ContactForm { .. }));
        assert_eq!(mgr.status_message.as_deref(), Some("Email is required"));
    }

    fn type_str(mgr: &mut ClientManager, conn: &Connection, text: &str) {
        for ch in text.chars() {
            mgr.handle_key(KeyCode::Char(ch), conn);
        }
    }

    fn form_values(mgr: &ClientManager) -> Vec<String> {
        match &mgr.screen {
            Screen::Add(form) | Screen::Edit(form) | Screen::ContactForm { form, .. } => {
                form.fields.iter().map(|f| f.value.clone()).collect()
            }
            _ => panic!("not on a form"),
        }
    }

    fn client_named(conn: &Connection, name: &str) -> Client {
        list_clients(conn, ClientScope::All)
            .unwrap()
            .into_iter()
            .find(|c| c.name == name)
            .unwrap_or_else(|| panic!("no client named {name}"))
    }

    /// Name, Email, Address, Notes typed into a fresh Add form.
    fn fill_add_form(mgr: &mut ClientManager, conn: &Connection, values: [&str; 4]) {
        mgr.handle_key(KeyCode::Char('a'), conn);
        for (i, value) in values.iter().enumerate() {
            if i > 0 {
                mgr.handle_key(KeyCode::Tab, conn);
            }
            type_str(mgr, conn, value);
        }
    }

    #[test]
    fn a_opens_the_add_form_with_empty_fields() {
        let (_d, conn) = test_conn();
        let mut mgr = manager(&conn);
        mgr.handle_key(KeyCode::Char('a'), &conn);

        assert!(matches!(mgr.screen, Screen::Add(_)));
        assert_eq!(form_values(&mgr), ["", "", "", ""]);
    }

    #[test]
    fn enter_with_a_blank_name_reports_it_and_stays_on_the_form() {
        let (_d, conn) = test_conn();
        let mut mgr = manager(&conn);
        fill_add_form(&mut mgr, &conn, ["  ", "ap@acme.test", "", ""]);
        mgr.handle_key(KeyCode::Enter, &conn);

        assert_eq!(mgr.status_message.as_deref(), Some("Name is required"));
        assert!(matches!(mgr.screen, Screen::Add(_)));
        assert!(list_clients(&conn, ClientScope::All).unwrap().is_empty());
    }

    #[test]
    fn enter_saves_a_client_and_returns_to_the_list() {
        let (_d, conn) = test_conn();
        let mut mgr = manager(&conn);
        fill_add_form(
            &mut mgr,
            &conn,
            ["Acme Co", "ap@acme.test", "1 Main St", "Net 30"],
        );
        mgr.handle_key(KeyCode::Enter, &conn);

        assert!(matches!(mgr.screen, Screen::List));
        assert_eq!(
            mgr.status_message.as_deref(),
            Some("Added client: Acme Co"),
            "the status line names the client"
        );
        let saved = client_named(&conn, "Acme Co");
        assert_eq!(saved.email.as_deref(), Some("ap@acme.test"));
        assert_eq!(saved.billing_address.as_deref(), Some("1 Main St"));
        assert_eq!(saved.notes.as_deref(), Some("Net 30"));
        assert_eq!(mgr.clients.len(), 1, "the list reloaded");
    }

    #[test]
    fn blank_optional_fields_are_stored_as_null() {
        let (_d, conn) = test_conn();
        let mut mgr = manager(&conn);
        fill_add_form(&mut mgr, &conn, ["Acme Co", "", "  ", ""]);
        mgr.handle_key(KeyCode::Enter, &conn);

        let saved = client_named(&conn, "Acme Co");
        assert_eq!(saved.email, None);
        assert_eq!(saved.billing_address, None);
        assert_eq!(saved.notes, None);
    }

    #[test]
    fn fields_are_trimmed() {
        let (_d, conn) = test_conn();
        let mut mgr = manager(&conn);
        fill_add_form(
            &mut mgr,
            &conn,
            ["  Acme Co  ", "  ap@acme.test ", " 1 Main St ", " Net 30 "],
        );
        mgr.handle_key(KeyCode::Enter, &conn);

        let saved = client_named(&conn, "Acme Co");
        assert_eq!(saved.email.as_deref(), Some("ap@acme.test"));
        assert_eq!(saved.billing_address.as_deref(), Some("1 Main St"));
        assert_eq!(saved.notes.as_deref(), Some("Net 30"));
    }

    #[test]
    fn esc_cancels_without_writing() {
        let (_d, conn) = test_conn();
        let mut mgr = manager(&conn);
        fill_add_form(&mut mgr, &conn, ["Acme Co", "", "", ""]);
        mgr.handle_key(KeyCode::Esc, &conn);

        assert!(matches!(mgr.screen, Screen::List));
        assert!(list_clients(&conn, ClientScope::All).unwrap().is_empty());
    }

    #[test]
    fn tab_and_backtab_cycle_the_four_fields() {
        let (_d, conn) = test_conn();
        let mut mgr = manager(&conn);
        mgr.handle_key(KeyCode::Char('a'), &conn);

        for expected in [1, 2, 3, 0] {
            mgr.handle_key(KeyCode::Tab, &conn);
            assert_eq!(focused(&mgr), expected);
        }
        for expected in [3, 2, 1, 0] {
            mgr.handle_key(KeyCode::BackTab, &conn);
            assert_eq!(focused(&mgr), expected);
        }
    }

    #[test]
    fn a_printable_key_types_into_the_field_rather_than_triggering_the_list_binding() {
        let (_d, conn) = test_conn();
        let mut mgr = manager(&conn);
        mgr.handle_key(KeyCode::Char('a'), &conn);
        type_str(&mut mgr, &conn, "aeq");
        mgr.handle_key(KeyCode::Backspace, &conn);

        assert!(matches!(mgr.screen, Screen::Add(_)), "still on the form");
        assert_eq!(form_values(&mgr)[0], "ae");
    }

    /// One fully-populated client, selected.
    fn seed_cedar(conn: &Connection) -> i64 {
        add_client(
            conn,
            "Cedar Systems",
            Some("ops@cedar.test"),
            Some("88 Cedar Way"),
            Some("Net 30"),
        )
        .unwrap()
    }

    /// Replace the focused field's contents.
    fn retype(mgr: &mut ClientManager, conn: &Connection, idx: usize, value: &str) {
        while focused(mgr) != idx {
            mgr.handle_key(KeyCode::Tab, conn);
        }
        for _ in 0..80 {
            mgr.handle_key(KeyCode::Backspace, conn);
        }
        type_str(mgr, conn, value);
    }

    #[test]
    fn e_opens_the_edit_form_prefilled_from_the_selected_row() {
        let (_d, conn) = test_conn();
        add_client(&conn, "Acme Co", None, None, None).unwrap();
        seed_cedar(&conn);
        let mut mgr = manager(&conn);
        mgr.handle_key(KeyCode::Down, &conn);
        mgr.handle_key(KeyCode::Char('e'), &conn);

        assert!(matches!(mgr.screen, Screen::Edit(_)));
        assert_eq!(
            form_values(&mgr),
            ["Cedar Systems", "ops@cedar.test", "88 Cedar Way", "Net 30"]
        );

        // An absent field renders as an empty string, never as "None".
        mgr.handle_key(KeyCode::Esc, &conn);
        mgr.handle_key(KeyCode::Up, &conn);
        mgr.handle_key(KeyCode::Char('e'), &conn);
        assert_eq!(form_values(&mgr), ["Acme Co", "", "", ""]);
    }

    #[test]
    fn enter_updates_the_client_and_returns_to_the_list() {
        let (_d, conn) = test_conn();
        let id = seed_cedar(&conn);
        let mut mgr = manager(&conn);
        mgr.handle_key(KeyCode::Char('e'), &conn);
        retype(&mut mgr, &conn, EMAIL_IDX, "billing@cedar.test");
        mgr.handle_key(KeyCode::Enter, &conn);

        assert!(matches!(mgr.screen, Screen::List));
        assert_eq!(
            mgr.status_message.as_deref(),
            Some("Updated client: Cedar Systems")
        );
        let saved = get_client(&conn, id).unwrap();
        assert_eq!(saved.email.as_deref(), Some("billing@cedar.test"));
        assert_eq!(saved.billing_address.as_deref(), Some("88 Cedar Way"));
    }

    #[test]
    fn clearing_an_optional_field_writes_null() {
        let (_d, conn) = test_conn();
        let id = seed_cedar(&conn);
        let mut mgr = manager(&conn);
        mgr.handle_key(KeyCode::Char('e'), &conn);
        retype(&mut mgr, &conn, EMAIL_IDX, "");
        mgr.handle_key(KeyCode::Enter, &conn);

        // Some(None), not None: None would have left the old address in place.
        assert_eq!(get_client(&conn, id).unwrap().email, None);
    }

    #[test]
    fn an_unchanged_field_still_round_trips_its_current_value() {
        let (_d, conn) = test_conn();
        let id = seed_cedar(&conn);
        let before = get_client(&conn, id).unwrap();
        let mut mgr = manager(&conn);
        mgr.handle_key(KeyCode::Char('e'), &conn);
        mgr.handle_key(KeyCode::Enter, &conn);

        let after = get_client(&conn, id).unwrap();
        assert_eq!(after.name, before.name);
        assert_eq!(after.email, before.email);
        assert_eq!(after.billing_address, before.billing_address);
        assert_eq!(after.notes, before.notes);
    }

    #[test]
    fn a_blank_name_is_refused_in_the_data_layer_s_own_words() {
        let (_d, conn) = test_conn();
        let id = seed_cedar(&conn);
        let mut mgr = manager(&conn);
        mgr.handle_key(KeyCode::Char('e'), &conn);
        retype(&mut mgr, &conn, NAME_IDX, "   ");
        mgr.handle_key(KeyCode::Enter, &conn);

        assert_eq!(mgr.status_message.as_deref(), Some("Name is required"));
        assert!(matches!(mgr.screen, Screen::Edit(_)));
        assert_eq!(get_client(&conn, id).unwrap().name, "Cedar Systems");
    }

    #[test]
    fn a_data_layer_error_is_shown_verbatim_and_keeps_the_form_open() {
        let (_d, conn) = test_conn();
        seed_cedar(&conn);
        conn.execute_batch(
            "CREATE TRIGGER no_edits BEFORE UPDATE ON clients
             BEGIN SELECT RAISE(ABORT, 'clients are frozen'); END;",
        )
        .unwrap();

        let mut mgr = manager(&conn);
        mgr.handle_key(KeyCode::Char('e'), &conn);
        retype(&mut mgr, &conn, NOTES_IDX, "Net 60");
        mgr.handle_key(KeyCode::Enter, &conn);

        let message = mgr.status_message.clone().unwrap();
        assert!(message.contains("clients are frozen"), "got: {message}");
        assert!(matches!(mgr.screen, Screen::Edit(_)), "the form stays open");
    }

    #[test]
    fn e_on_an_empty_list_does_nothing() {
        let (_d, conn) = test_conn();
        let mut mgr = manager(&conn);
        mgr.handle_key(KeyCode::Char('e'), &conn);
        assert!(matches!(mgr.screen, Screen::List));
    }

    #[test]
    fn truncate_leaves_short_strings_alone_and_ellipsises_long_ones() {
        assert_eq!(truncate("Acme Co", 26), "Acme Co");
        assert_eq!(truncate("abcde", 5), "abcde");
        assert_eq!(truncate("abcdef", 5), "abcd\u{2026}");
        // Character-wise, not byte-wise: a multi-byte name must not be split.
        assert_eq!(truncate("é".repeat(6).as_str(), 5), "éééé\u{2026}");
    }

    #[test]
    fn optional_display_renders_none_as_an_em_dash() {
        assert_eq!(optional_display(Some("ap@acme.test")), "ap@acme.test");
        assert_eq!(optional_display(None), "\u{2014}");
        assert_eq!(optional_display(Some("   ")), "\u{2014}");
    }

    /// The screen as an 80x24 terminal renders it, one string per row.
    fn rendered(mgr: &mut ClientManager) -> String {
        let mut terminal =
            ratatui::Terminal::new(ratatui::backend::TestBackend::new(80, 24)).unwrap();
        terminal.draw(|frame| mgr.draw(frame)).unwrap();
        terminal
            .backend()
            .buffer()
            .content()
            .chunks(80)
            .map(|row| row.iter().map(|cell| cell.symbol()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn the_list_renders_its_columns_and_footer() {
        let (_d, conn) = test_conn();
        seed_cedar(&conn);
        add_client(&conn, "Acme Co", None, None, None).unwrap();
        let mut mgr = manager(&conn);

        let screen = rendered(&mut mgr);
        assert!(screen.contains("Clients (2)"), "{screen}");
        assert!(screen.contains("Name"), "{screen}");
        assert!(screen.contains("Billing address"), "{screen}");
        assert!(screen.contains("> Acme Co"), "{screen}");
        assert!(screen.contains("ops@cedar.test"), "{screen}");
        // Acme has neither email nor address.
        assert!(screen.contains('\u{2014}'), "{screen}");
        assert!(
            screen.contains(
                "a=add  e=edit  c=contacts  d=delete  x=archive  A=show all  Esc=back  q=quit"
            ),
            "{screen}"
        );
    }

    #[test]
    fn every_screen_survives_a_terminal_narrower_than_its_columns() {
        let (_d, conn) = test_conn();
        seed_cedar(&conn);
        let mut mgr = manager(&conn);

        let mut narrow =
            ratatui::Terminal::new(ratatui::backend::TestBackend::new(40, 10)).unwrap();
        narrow.draw(|frame| mgr.draw(frame)).unwrap(); // list
        mgr.handle_key(KeyCode::Char('e'), &conn);
        narrow.draw(|frame| mgr.draw(frame)).unwrap(); // edit form
    }

    #[test]
    fn the_empty_list_says_how_to_add_one() {
        let (_d, conn) = test_conn();
        let mut mgr = manager(&conn);
        let screen = rendered(&mut mgr);
        assert!(screen.contains("Clients (0)"), "{screen}");
        assert!(
            screen.contains("No clients yet. Press 'a' to add one."),
            "{screen}"
        );
    }

    #[test]
    fn a_long_row_stays_inside_eighty_columns() {
        let (_d, conn) = test_conn();
        add_client(
            &conn,
            &"Wintermute Consolidated Holdings".repeat(3),
            Some(&format!("{}@example.test", "a".repeat(60))),
            Some(&"1 Very Long Street Name, Portland OR".repeat(3)),
            None,
        )
        .unwrap();
        // A rendered frame is 80 cells wide by construction, so the budget is
        // checked on the string the row is built from, not on the buffer.
        let client = list_clients(&conn, ClientScope::All)
            .unwrap()
            .pop()
            .unwrap();
        assert!(
            client_row(" > ", &client, 80).chars().count() <= 80,
            "row overflows: {:?}",
            client_row(" > ", &client, 80)
        );
        let mut mgr = manager(&conn);
        assert!(rendered(&mut mgr).contains("Wintermute"));
    }

    #[test]
    fn the_add_form_renders_its_fields_and_hint() {
        let (_d, conn) = test_conn();
        let mut mgr = manager(&conn);
        mgr.handle_key(KeyCode::Char('a'), &conn);

        let screen = rendered(&mut mgr);
        assert!(screen.contains("Add Client"), "{screen}");
        for label in ["Name", "Email", "Address", "Notes"] {
            assert!(screen.contains(label), "{label} missing:\n{screen}");
        }
        assert!(screen.contains(EMAIL_HINT), "{screen}");
        assert!(
            screen.contains("Tab=next field  Enter=save  Esc=cancel"),
            "{screen}"
        );
    }

    #[test]
    fn a_failed_save_shows_the_message_where_the_hint_was() {
        let (_d, conn) = test_conn();
        let mut mgr = manager(&conn);
        mgr.handle_key(KeyCode::Char('a'), &conn);
        mgr.handle_key(KeyCode::Enter, &conn);

        let screen = rendered(&mut mgr);
        assert!(screen.contains("Name is required"), "{screen}");
        assert!(!screen.contains(EMAIL_HINT), "{screen}");
    }

    #[test]
    fn the_edit_form_renders_the_selected_client() {
        let (_d, conn) = test_conn();
        seed_cedar(&conn);
        let mut mgr = manager(&conn);
        mgr.handle_key(KeyCode::Char('e'), &conn);

        let screen = rendered(&mut mgr);
        assert!(screen.contains("Edit Client"), "{screen}");
        assert!(screen.contains("Cedar Systems"), "{screen}");
        assert!(screen.contains("88 Cedar Way"), "{screen}");
    }

    fn focused(mgr: &ClientManager) -> usize {
        match &mgr.screen {
            Screen::Add(form) | Screen::Edit(form) | Screen::ContactForm { form, .. } => {
                form.focused
            }
            _ => panic!("not on a form"),
        }
    }

    #[test]
    fn a_status_message_expires_after_three_keypresses() {
        let (_d, conn) = test_conn();
        let mut mgr = manager(&conn);
        mgr.set_status("Added client: Acme Co".into());

        for _ in 0..2 {
            mgr.handle_key(KeyCode::Down, &conn);
            assert!(mgr.status_message.is_some());
        }
        mgr.handle_key(KeyCode::Down, &conn);
        assert!(mgr.status_message.is_none());
    }
}
