use crossterm::event::KeyCode;
use ratatui::{
    layout::{Constraint, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};
use rusqlite::Connection;

use crate::cli::password_manager::{PasswordAction, PasswordManager};
use crate::db;
use crate::error::Result;
use crate::settings::{get_data_dir, load_settings, save_settings};
use crate::tui::{FOOTER_STYLE, HEADER_STYLE, SELECTED_STYLE};

pub enum SettingsAction {
    Continue,
    Close,
}

enum Screen {
    Main,
    /// Editing one of the letterhead rows, keyed by its `MENU_*` constant.
    Editing(usize),
    Password(PasswordManager),
}

/// Menu items on the main settings screen.
const MENU_BUSINESS_NAME: usize = 0;
const MENU_COMPANY_ADDRESS: usize = 1;
const MENU_COMPANY_PHONE: usize = 2;
const MENU_COMPANY_LOGO: usize = 3;
const MENU_PAYMENT_INSTRUCTIONS: usize = 4;
const MENU_PASSWORD: usize = 5;
const MENU_UPDATE_CHECK: usize = 6;
const MENU_LAST: usize = MENU_UPDATE_CHECK;

/// The metadata key each editable row writes.
fn metadata_key(row: usize) -> Option<&'static str> {
    match row {
        MENU_BUSINESS_NAME => Some("company_name"),
        MENU_COMPANY_ADDRESS => Some("company_address"),
        MENU_COMPANY_PHONE => Some("company_phone"),
        MENU_COMPANY_LOGO => Some("company_logo"),
        MENU_PAYMENT_INSTRUCTIONS => Some("payment_instructions"),
        _ => None,
    }
}

/// The rows whose value is typed over more than one line.
///
/// This form has one single-line buffer per field and no multi-line widget, so
/// these two take `\n` as the two-character escape `\n` and store real
/// newlines. It is the smallest convention that lets a two-line address be
/// typed here at all, and it is applied to these fields and nothing else.
fn is_multiline(row: usize) -> bool {
    matches!(row, MENU_COMPANY_ADDRESS | MENU_PAYMENT_INSTRUCTIONS)
}

/// The escape and its inverse. The backslash is escaped too, so the pair is a
/// true round trip: without it a stored value carrying a literal `\n` would be
/// rewritten into a real line break the next time the field was opened and
/// saved, and there would be no way to type one at all.
fn escape_newlines(value: &str) -> String {
    value.replace('\\', "\\\\").replace('\n', "\\n")
}

fn unescape_newlines(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut chars = value.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('\\') => out.push('\\'),
            // A lone backslash before anything else is what was typed, so it is
            // what is stored: this form invents no vocabulary beyond the two
            // sequences it documents.
            Some(other) => {
                out.push('\\');
                out.push(other);
            }
            None => out.push('\\'),
        }
    }
    out
}

pub struct SettingsManager {
    greeting: String,
    screen: Screen,
    selection: usize,
    company_name: String,
    company_address: String,
    company_phone: String,
    /// The stored `data:` URI. Never shown — the row reports whether one is set.
    company_logo: String,
    payment_instructions: String,
    edit_buffer: String,
    status_message: Option<(String, bool)>,
    status_ttl: u8,
    encrypted: bool,
    update_check: bool,
    /// "Business Name" or "Household Name", from the database's profile.
    name_label: &'static str,
}

impl SettingsManager {
    pub fn new(conn: &Connection, greeting: &str) -> Result<Self> {
        let company_name = db::get_metadata(conn, "company_name").unwrap_or_default();
        let db_path = get_data_dir().join("nigel.db");
        let encrypted = db::is_encrypted(&db_path)?;
        let settings = load_settings();
        let name_label = match db::get_profile(conn) {
            db::Profile::Business => "Business Name",
            db::Profile::Personal => "Household Name",
        };
        let read = |key: &str| db::get_metadata(conn, key).unwrap_or_default();
        Ok(Self {
            greeting: greeting.to_string(),
            screen: Screen::Main,
            selection: 0,
            company_name,
            company_address: read("company_address"),
            company_phone: read("company_phone"),
            company_logo: read("company_logo"),
            payment_instructions: read("payment_instructions"),
            edit_buffer: String::new(),
            status_message: None,
            status_ttl: 0,
            encrypted,
            update_check: settings.update_check,
            name_label,
        })
    }

    fn set_status(&mut self, msg: String, success: bool) {
        self.status_message = Some((msg, success));
        self.status_ttl = 3;
    }

    fn tick_status(&mut self) {
        if self.status_ttl > 0 {
            self.status_ttl -= 1;
            if self.status_ttl == 0 {
                self.status_message = None;
            }
        }
    }

    pub fn draw(&self, frame: &mut Frame) {
        match &self.screen {
            Screen::Main | Screen::Editing(_) => self.draw_main(frame),
            Screen::Password(mgr) => mgr.draw(frame),
        }
    }

    /// The label and the value each letterhead row shows when it is not being
    /// edited. Multi-line values are shown with their `\n` escapes, which is
    /// how they are typed.
    fn row_display(&self, row: usize) -> (&str, String) {
        let empty = "(not set)".to_string();
        match row {
            MENU_BUSINESS_NAME => (
                self.name_label,
                if self.company_name.is_empty() {
                    empty
                } else {
                    self.company_name.clone()
                },
            ),
            MENU_COMPANY_ADDRESS => (
                "Address",
                if self.company_address.is_empty() {
                    empty
                } else {
                    escape_newlines(&self.company_address)
                },
            ),
            MENU_COMPANY_PHONE => (
                "Phone",
                if self.company_phone.is_empty() {
                    empty
                } else {
                    self.company_phone.clone()
                },
            ),
            // A data URI is thousands of characters of base64; the row says
            // whether there is one, and the field takes a path to replace it.
            MENU_COMPANY_LOGO => (
                "Logo",
                if self.company_logo.is_empty() {
                    empty
                } else {
                    "(set)".to_string()
                },
            ),
            _ => (
                "Payment info",
                if self.payment_instructions.is_empty() {
                    empty
                } else {
                    escape_newlines(&self.payment_instructions)
                },
            ),
        }
    }

    /// What the edit buffer starts as. The logo's is empty: its stored value is
    /// a data URI and what is typed here is a path to an image file.
    fn edit_seed(&self, row: usize) -> String {
        match row {
            MENU_COMPANY_LOGO => String::new(),
            _ => escape_newlines(&self.row_value(row)),
        }
    }

    fn row_value(&self, row: usize) -> String {
        match row {
            MENU_BUSINESS_NAME => self.company_name.clone(),
            MENU_COMPANY_ADDRESS => self.company_address.clone(),
            MENU_COMPANY_PHONE => self.company_phone.clone(),
            MENU_PAYMENT_INSTRUCTIONS => self.payment_instructions.clone(),
            _ => String::new(),
        }
    }

    fn menu_row(label: &str, value: &str, selected: bool) -> Line<'static> {
        let marker = if selected { ">" } else { " " };
        let label_style = if selected {
            Style::default().add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };
        Line::from(vec![
            Span::styled(format!(" {marker} {label:<17}"), label_style),
            Span::styled(
                value.to_string(),
                if selected {
                    Style::default().fg(Color::Cyan)
                } else {
                    Style::default().fg(Color::DarkGray)
                },
            ),
        ])
    }

    fn draw_main(&self, frame: &mut Frame) {
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

        // Content
        let mut lines = vec![
            Line::from(""),
            Line::from(Span::styled(
                " Settings",
                Style::default().add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
        ];

        // The letterhead rows. Whichever one is being edited shows its buffer
        // in place of its value; the rest read as ordinary menu rows.
        for row in MENU_BUSINESS_NAME..=MENU_PAYMENT_INSTRUCTIONS {
            let selected = self.selection == row;
            let (label, value) = self.row_display(row);
            if matches!(self.screen, Screen::Editing(editing) if editing == row) {
                let marker = if selected { ">" } else { " " };
                let label_style = if selected {
                    Style::default().add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };
                lines.push(Line::from(vec![
                    Span::styled(format!(" {marker} {label:<17}"), label_style),
                    Span::styled(format!("{}_", self.edit_buffer), SELECTED_STYLE),
                ]));
            } else {
                lines.push(Self::menu_row(label, &value, selected));
            }
        }

        lines.push(Line::from(""));

        // Password section
        let pw_status = if self.encrypted {
            "(encrypted)"
        } else {
            "(not set)"
        };
        lines.push(Self::menu_row(
            "Password",
            pw_status,
            self.selection == MENU_PASSWORD,
        ));

        lines.push(Line::from(""));

        // Update check toggle
        let uc_status = if self.update_check {
            "(enabled)"
        } else {
            "(disabled)"
        };
        lines.push(Self::menu_row(
            "Auto-update check",
            uc_status,
            self.selection == MENU_UPDATE_CHECK,
        ));

        // Status message
        if let Some((msg, success)) = &self.status_message {
            lines.push(Line::from(""));
            let color = if *success { Color::Green } else { Color::Red };
            lines.push(Line::from(Span::styled(
                format!("   {msg}"),
                Style::default().fg(color),
            )));
        }

        frame.render_widget(Paragraph::new(lines), content_area);

        // Hints
        let hints = match &self.screen {
            Screen::Editing(MENU_COMPANY_LOGO) => {
                "Enter=save  Esc=cancel  (path to a PNG or JPEG; empty clears)"
            }
            Screen::Editing(row) if is_multiline(*row) => {
                "Enter=save  Esc=cancel  (\\n starts a new line)"
            }
            Screen::Editing(_) => "Enter=save  Esc=cancel",
            _ => "Enter=select  Esc=back  q=quit",
        };
        frame.render_widget(
            Paragraph::new(format!(" {hints}")).style(FOOTER_STYLE),
            hints_area,
        );
    }

    pub fn handle_key(&mut self, code: KeyCode, conn: &Connection) -> SettingsAction {
        self.tick_status();

        match &mut self.screen {
            Screen::Main => self.handle_main_key(code, conn),
            Screen::Editing(row) => {
                let row = *row;
                self.handle_edit_key(code, conn, row)
            }
            Screen::Password(mgr) => {
                match mgr.handle_key(code) {
                    PasswordAction::Close => {
                        // Refresh encrypted status when returning from password manager
                        let db_path = get_data_dir().join("nigel.db");
                        match db::is_encrypted(&db_path) {
                            Ok(enc) => self.encrypted = enc,
                            Err(e) => {
                                // Preserve previous state rather than defaulting to false
                                self.set_status(
                                    format!("Could not verify encryption status: {e}"),
                                    false,
                                );
                            }
                        }
                        self.screen = Screen::Main;
                    }
                    PasswordAction::Continue => {}
                }
                SettingsAction::Continue
            }
        }
    }

    fn handle_main_key(&mut self, code: KeyCode, _conn: &Connection) -> SettingsAction {
        match code {
            KeyCode::Esc | KeyCode::Char('q') => SettingsAction::Close,
            KeyCode::Up => {
                self.selection = self.selection.saturating_sub(1);
                SettingsAction::Continue
            }
            KeyCode::Down => {
                self.selection = (self.selection + 1).min(MENU_LAST);
                SettingsAction::Continue
            }
            KeyCode::Enter => {
                match self.selection {
                    row if metadata_key(row).is_some() => {
                        self.edit_buffer = self.edit_seed(row);
                        self.screen = Screen::Editing(row);
                    }
                    MENU_PASSWORD => match PasswordManager::new(&self.greeting) {
                        Ok(mgr) => self.screen = Screen::Password(mgr),
                        Err(e) => {
                            self.set_status(format!("Could not open password settings: {e}"), false)
                        }
                    },
                    MENU_UPDATE_CHECK => {
                        self.update_check = !self.update_check;
                        let mut settings = load_settings();
                        settings.update_check = self.update_check;
                        match save_settings(&settings) {
                            Ok(()) => {
                                let state = if self.update_check {
                                    "enabled"
                                } else {
                                    "disabled"
                                };
                                self.set_status(format!("Auto-update check {state}."), true);
                            }
                            Err(e) => {
                                // Revert on save failure
                                self.update_check = !self.update_check;
                                self.set_status(format!("Could not save setting: {e}"), false);
                            }
                        }
                    }
                    _ => {}
                }
                SettingsAction::Continue
            }
            _ => SettingsAction::Continue,
        }
    }

    fn handle_edit_key(&mut self, code: KeyCode, conn: &Connection, row: usize) -> SettingsAction {
        match code {
            KeyCode::Esc => {
                self.edit_buffer.clear();
                self.screen = Screen::Main;
            }
            KeyCode::Enter => {
                self.save_row(conn, row);
                self.edit_buffer.clear();
                self.screen = Screen::Main;
            }
            KeyCode::Char(c) => {
                self.edit_buffer.push(c);
            }
            KeyCode::Backspace => {
                self.edit_buffer.pop();
            }
            _ => {}
        }
        SettingsAction::Continue
    }

    fn save_row(&mut self, conn: &Connection, row: usize) {
        let Some(key) = metadata_key(row) else {
            return;
        };
        let typed = self.edit_buffer.trim().to_string();

        let value = if row == MENU_COMPANY_LOGO {
            match self.logo_value(&typed) {
                Ok(value) => value,
                // A refusal is the status line's, and the stored key is
                // untouched: a bad file is refused here rather than in a
                // client's inbox.
                Err(message) => return self.set_status(message, false),
            }
        } else if is_multiline(row) {
            unescape_newlines(&typed)
        } else {
            typed
        };

        match db::set_metadata(conn, key, &value) {
            Ok(()) => {
                match row {
                    MENU_BUSINESS_NAME => self.company_name = value,
                    MENU_COMPANY_ADDRESS => self.company_address = value,
                    MENU_COMPANY_PHONE => self.company_phone = value,
                    MENU_COMPANY_LOGO => self.company_logo = value,
                    _ => self.payment_instructions = value,
                }
                let (label, _) = self.row_display(row);
                self.set_status(format!("{label} saved."), true);
            }
            Err(e) => self.set_status(format!("Could not save: {e}"), false),
        }
    }

    /// The data URI a typed path becomes, or the reason it cannot be one. An
    /// empty path clears the logo, the way an empty field clears every other
    /// row here.
    fn logo_value(&self, path: &str) -> std::result::Result<String, String> {
        use base64::Engine as _;

        if path.is_empty() {
            return Ok(String::new());
        }
        let expanded = crate::settings::shellexpand_path(path);
        let bytes = std::fs::read(&expanded).map_err(|e| format!("Could not read {path}: {e}"))?;
        // The MIME is declared from the bytes, and `parse_logo` then checks the
        // bytes against it — so a `.png` holding a JPEG is refused rather than
        // mislabelled into every email body.
        let mime = if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
            "image/jpeg"
        } else {
            "image/png"
        };
        let uri = format!(
            "data:{mime};base64,{}",
            base64::engine::general_purpose::STANDARD.encode(&bytes)
        );
        match crate::invoicing::document::parse_logo(&uri) {
            Ok(Some(_)) => Ok(uri),
            Ok(None) => Err(format!("{path} is empty.")),
            Err(e) => Err(e.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_db() -> (tempfile::TempDir, Connection) {
        let dir = tempfile::tempdir().unwrap();
        let conn = db::get_connection(&dir.path().join("test.db")).unwrap();
        db::init_db(&conn).unwrap();
        (dir, conn)
    }

    #[test]
    fn name_label_follows_the_books_profile() {
        let (_dir, conn) = test_db();
        let mgr = SettingsManager::new(&conn, "Hello").unwrap();
        assert_eq!(mgr.name_label, "Business Name");

        let dir = tempfile::tempdir().unwrap();
        let conn = db::get_connection(&dir.path().join("personal.db")).unwrap();
        db::init_db_with_profile(&conn, db::Profile::Personal).unwrap();
        let mgr = SettingsManager::new(&conn, "Hello").unwrap();
        assert_eq!(mgr.name_label, "Household Name");
    }

    #[test]
    fn new_loads_company_name() {
        let (_dir, conn) = test_db();
        db::set_metadata(&conn, "company_name", "Acme LLC").unwrap();
        let mgr = SettingsManager::new(&conn, "Hello").unwrap();
        assert_eq!(mgr.company_name, "Acme LLC");
    }

    #[test]
    fn new_with_no_company_name() {
        let (_dir, conn) = test_db();
        let mgr = SettingsManager::new(&conn, "Hello").unwrap();
        assert_eq!(mgr.company_name, "");
    }

    #[test]
    fn esc_closes() {
        let (_dir, conn) = test_db();
        let mut mgr = SettingsManager::new(&conn, "Hello").unwrap();
        let action = mgr.handle_key(KeyCode::Esc, &conn);
        assert!(matches!(action, SettingsAction::Close));
    }

    #[test]
    fn q_closes() {
        let (_dir, conn) = test_db();
        let mut mgr = SettingsManager::new(&conn, "Hello").unwrap();
        let action = mgr.handle_key(KeyCode::Char('q'), &conn);
        assert!(matches!(action, SettingsAction::Close));
    }

    #[test]
    fn navigate_menu() {
        let (_dir, conn) = test_db();
        let mut mgr = SettingsManager::new(&conn, "Hello").unwrap();
        assert_eq!(mgr.selection, MENU_BUSINESS_NAME);
        for expected in [
            MENU_COMPANY_ADDRESS,
            MENU_COMPANY_PHONE,
            MENU_COMPANY_LOGO,
            MENU_PAYMENT_INSTRUCTIONS,
            MENU_PASSWORD,
            MENU_UPDATE_CHECK,
        ] {
            mgr.handle_key(KeyCode::Down, &conn);
            assert_eq!(mgr.selection, expected);
        }
        mgr.handle_key(KeyCode::Down, &conn);
        assert_eq!(mgr.selection, MENU_LAST, "clamped to the new last row");
        mgr.handle_key(KeyCode::Up, &conn);
        assert_eq!(mgr.selection, MENU_PASSWORD);
    }

    /// The rows the letterhead added, between the name and the password.
    #[test]
    fn the_settings_screen_lists_address_phone_logo_and_payment_instructions_under_the_name() {
        let (_dir, conn) = test_db();
        let mgr = SettingsManager::new(&conn, "Hello").unwrap();
        let labels: Vec<&str> = (MENU_BUSINESS_NAME..=MENU_PAYMENT_INSTRUCTIONS)
            .map(|row| mgr.row_display(row).0)
            .collect();
        assert_eq!(
            labels,
            vec!["Business Name", "Address", "Phone", "Logo", "Payment info"]
        );
        const _: () = assert!(
            MENU_PAYMENT_INSTRUCTIONS < MENU_PASSWORD,
            "the letterhead rows sit above the password"
        );
    }

    /// Enter, type, Enter — on whichever row is selected.
    fn edit_row(mgr: &mut SettingsManager, conn: &Connection, row: usize, typed: &str) {
        mgr.selection = row;
        mgr.handle_key(KeyCode::Enter, conn);
        for _ in 0..mgr.edit_buffer.chars().count() {
            mgr.handle_key(KeyCode::Backspace, conn);
        }
        for c in typed.chars() {
            mgr.handle_key(KeyCode::Char(c), conn);
        }
        mgr.handle_key(KeyCode::Enter, conn);
    }

    #[test]
    fn editing_the_address_saves_to_the_company_address_key() {
        let (_dir, conn) = test_db();
        let mut mgr = SettingsManager::new(&conn, "Hello").unwrap();
        edit_row(&mut mgr, &conn, MENU_COMPANY_ADDRESS, "P.O. Box 1234");
        assert_eq!(
            db::get_metadata(&conn, "company_address").unwrap(),
            "P.O. Box 1234"
        );
        assert_eq!(mgr.company_address, "P.O. Box 1234");
    }

    #[test]
    fn editing_the_payment_instructions_saves_to_its_own_key() {
        let (_dir, conn) = test_db();
        let mut mgr = SettingsManager::new(&conn, "Hello").unwrap();
        edit_row(&mut mgr, &conn, MENU_PAYMENT_INSTRUCTIONS, "Wells Fargo");
        assert_eq!(
            db::get_metadata(&conn, "payment_instructions").unwrap(),
            "Wells Fargo"
        );
    }

    /// This form has one single-line buffer per field, so the escape is the
    /// only way a two-line value can be typed here at all.
    #[test]
    fn a_backslash_n_in_a_multiline_field_stores_a_real_newline() {
        let (_dir, conn) = test_db();
        let mut mgr = SettingsManager::new(&conn, "Hello").unwrap();
        edit_row(
            &mut mgr,
            &conn,
            MENU_COMPANY_ADDRESS,
            "P.O. Box 1234\\nSpringfield, CA 90001",
        );
        assert_eq!(
            db::get_metadata(&conn, "company_address").unwrap(),
            "P.O. Box 1234\nSpringfield, CA 90001"
        );

        edit_row(
            &mut mgr,
            &conn,
            MENU_PAYMENT_INSTRUCTIONS,
            "Wells Fargo\\nRouting 121000248",
        );
        assert_eq!(
            db::get_metadata(&conn, "payment_instructions").unwrap(),
            "Wells Fargo\nRouting 121000248"
        );
    }

    /// The escape has to be reversible or it is a data-loss bug: a value
    /// carrying a literal backslash-n would come back as a real line break the
    /// next time the field was opened and saved, and again, and again.
    #[test]
    fn a_value_holding_a_literal_escape_survives_a_form_round_trip() {
        let (_dir, conn) = test_db();
        let mut mgr = SettingsManager::new(&conn, "Hello").unwrap();
        // Typed as `\\n`, which is how the form spells one literal backslash
        // followed by an n.
        edit_row(
            &mut mgr,
            &conn,
            MENU_PAYMENT_INSTRUCTIONS,
            r"Reference C:\\name on the wire",
        );
        let stored = db::get_metadata(&conn, "payment_instructions").unwrap();
        assert_eq!(stored, r"Reference C:\name on the wire");

        // Reopen and save again without touching anything: the value must not
        // move.
        let mut mgr = SettingsManager::new(&conn, "Hello").unwrap();
        mgr.selection = MENU_PAYMENT_INSTRUCTIONS;
        mgr.handle_key(KeyCode::Enter, &conn);
        assert_eq!(mgr.edit_buffer, r"Reference C:\\name on the wire");
        mgr.handle_key(KeyCode::Enter, &conn);
        assert_eq!(
            db::get_metadata(&conn, "payment_instructions").unwrap(),
            stored,
            "a round trip changed the stored value"
        );
    }

    #[test]
    fn reopening_a_multiline_field_shows_the_escape_again_not_a_raw_newline() {
        let (_dir, conn) = test_db();
        db::set_metadata(&conn, "company_address", "One\nTwo").unwrap();
        let mut mgr = SettingsManager::new(&conn, "Hello").unwrap();
        mgr.selection = MENU_COMPANY_ADDRESS;
        mgr.handle_key(KeyCode::Enter, &conn);
        assert_eq!(mgr.edit_buffer, "One\\nTwo");
        assert_eq!(mgr.row_display(MENU_COMPANY_ADDRESS).1, "One\\nTwo");
    }

    #[test]
    fn an_empty_multiline_field_clears_the_key() {
        let (_dir, conn) = test_db();
        db::set_metadata(&conn, "company_address", "One\nTwo").unwrap();
        let mut mgr = SettingsManager::new(&conn, "Hello").unwrap();
        edit_row(&mut mgr, &conn, MENU_COMPANY_ADDRESS, "");
        assert_eq!(db::get_metadata(&conn, "company_address").unwrap(), "");
        assert_eq!(mgr.row_display(MENU_COMPANY_ADDRESS).1, "(not set)");
    }

    fn write_png(dir: &std::path::Path, name: &str) -> String {
        let png: &[u8] = &[
            0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, b'I', b'H',
            b'D', b'R', 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, b'I', b'E', b'N', b'D', 0xae,
            0x42, 0x60, 0x82,
        ];
        let path = dir.join(name);
        std::fs::write(&path, png).unwrap();
        path.to_string_lossy().into_owned()
    }

    #[test]
    fn the_logo_field_takes_a_path_and_stores_a_data_uri() {
        let (dir, conn) = test_db();
        let path = write_png(dir.path(), "logo.png");
        let mut mgr = SettingsManager::new(&conn, "Hello").unwrap();
        edit_row(&mut mgr, &conn, MENU_COMPANY_LOGO, &path);

        let stored = db::get_metadata(&conn, "company_logo").unwrap();
        assert!(
            stored.starts_with("data:image/png;base64,"),
            "got: {stored}"
        );
        assert!(crate::invoicing::document::parse_logo(&stored)
            .unwrap()
            .is_some());
        assert_eq!(mgr.row_display(MENU_COMPANY_LOGO).1, "(set)");
    }

    #[test]
    fn a_logo_that_is_not_a_png_or_jpeg_is_refused_on_the_status_line() {
        let (dir, conn) = test_db();
        let path = dir.path().join("logo.png");
        std::fs::write(&path, b"<svg></svg>").unwrap();
        db::set_metadata(&conn, "company_logo", "data:image/png;base64,keepme").unwrap();
        let mut mgr = SettingsManager::new(&conn, "Hello").unwrap();
        edit_row(&mut mgr, &conn, MENU_COMPANY_LOGO, &path.to_string_lossy());

        let (message, ok) = mgr.status_message.clone().expect("a refusal");
        assert!(!ok, "got: {message}");
        assert!(message.contains("PNG"), "got: {message}");
        assert_eq!(
            db::get_metadata(&conn, "company_logo").unwrap(),
            "data:image/png;base64,keepme",
            "the stored key is untouched"
        );
    }

    #[test]
    fn a_missing_logo_file_is_refused_by_name() {
        let (dir, conn) = test_db();
        let path = dir.path().join("nope.png");
        let mut mgr = SettingsManager::new(&conn, "Hello").unwrap();
        edit_row(&mut mgr, &conn, MENU_COMPANY_LOGO, &path.to_string_lossy());

        let (message, ok) = mgr.status_message.clone().expect("a refusal");
        assert!(!ok);
        assert!(message.contains("nope.png"), "got: {message}");
    }

    #[test]
    fn an_empty_logo_field_clears_the_logo() {
        let (dir, conn) = test_db();
        let path = write_png(dir.path(), "logo.png");
        let mut mgr = SettingsManager::new(&conn, "Hello").unwrap();
        edit_row(&mut mgr, &conn, MENU_COMPANY_LOGO, &path);
        edit_row(&mut mgr, &conn, MENU_COMPANY_LOGO, "");
        assert_eq!(db::get_metadata(&conn, "company_logo").unwrap(), "");
    }

    #[test]
    fn edit_business_name_save() {
        let (_dir, conn) = test_db();
        let mut mgr = SettingsManager::new(&conn, "Hello").unwrap();

        // Enter edit mode
        mgr.handle_key(KeyCode::Enter, &conn);
        assert!(matches!(mgr.screen, Screen::Editing(MENU_BUSINESS_NAME)));

        // Type a name
        for c in "Test Corp".chars() {
            mgr.handle_key(KeyCode::Char(c), &conn);
        }
        assert_eq!(mgr.edit_buffer, "Test Corp");

        // Save
        mgr.handle_key(KeyCode::Enter, &conn);
        assert!(matches!(mgr.screen, Screen::Main));
        assert_eq!(mgr.company_name, "Test Corp");

        // Verify persisted
        let saved = db::get_metadata(&conn, "company_name").unwrap();
        assert_eq!(saved, "Test Corp");
    }

    #[test]
    fn edit_business_name_cancel() {
        let (_dir, conn) = test_db();
        db::set_metadata(&conn, "company_name", "Original").unwrap();
        let mut mgr = SettingsManager::new(&conn, "Hello").unwrap();

        // Enter edit mode
        mgr.handle_key(KeyCode::Enter, &conn);
        for c in "Changed".chars() {
            mgr.handle_key(KeyCode::Char(c), &conn);
        }

        // Cancel
        mgr.handle_key(KeyCode::Esc, &conn);
        assert!(matches!(mgr.screen, Screen::Main));
        assert_eq!(mgr.company_name, "Original");

        // Verify DB unchanged
        let saved = db::get_metadata(&conn, "company_name").unwrap();
        assert_eq!(saved, "Original");
    }

    #[test]
    fn edit_business_name_backspace() {
        let (_dir, conn) = test_db();
        let mut mgr = SettingsManager::new(&conn, "Hello").unwrap();

        mgr.handle_key(KeyCode::Enter, &conn);
        for c in "ABC".chars() {
            mgr.handle_key(KeyCode::Char(c), &conn);
        }
        mgr.handle_key(KeyCode::Backspace, &conn);
        assert_eq!(mgr.edit_buffer, "AB");
    }

    #[test]
    fn enter_password_screen() {
        let (_dir, conn) = test_db();
        let mut mgr = SettingsManager::new(&conn, "Hello").unwrap();

        // Navigate to password
        mgr.selection = MENU_PASSWORD;

        // Enter password manager
        mgr.handle_key(KeyCode::Enter, &conn);
        assert!(matches!(mgr.screen, Screen::Password(_)));

        // Esc returns to main
        mgr.handle_key(KeyCode::Esc, &conn);
        assert!(matches!(mgr.screen, Screen::Main));
    }

    #[test]
    fn edit_trims_whitespace() {
        let (_dir, conn) = test_db();
        let mut mgr = SettingsManager::new(&conn, "Hello").unwrap();

        mgr.handle_key(KeyCode::Enter, &conn);
        for c in "  Acme LLC  ".chars() {
            mgr.handle_key(KeyCode::Char(c), &conn);
        }
        mgr.handle_key(KeyCode::Enter, &conn);
        assert_eq!(mgr.company_name, "Acme LLC");
        assert_eq!(db::get_metadata(&conn, "company_name").unwrap(), "Acme LLC");
    }

    #[test]
    fn edit_empty_name_saves() {
        let (_dir, conn) = test_db();
        db::set_metadata(&conn, "company_name", "Old Name").unwrap();
        let mut mgr = SettingsManager::new(&conn, "Hello").unwrap();

        // Enter edit, clear, save empty
        mgr.handle_key(KeyCode::Enter, &conn);
        // Buffer is pre-populated; clear it
        for _ in 0..mgr.edit_buffer.len() {
            mgr.handle_key(KeyCode::Backspace, &conn);
        }
        mgr.handle_key(KeyCode::Enter, &conn);
        assert_eq!(mgr.company_name, "");
    }

    #[test]
    fn edit_prepopulates_buffer() {
        let (_dir, conn) = test_db();
        db::set_metadata(&conn, "company_name", "Existing Corp").unwrap();
        let mut mgr = SettingsManager::new(&conn, "Hello").unwrap();

        mgr.handle_key(KeyCode::Enter, &conn);
        assert!(matches!(mgr.screen, Screen::Editing(MENU_BUSINESS_NAME)));
        assert_eq!(mgr.edit_buffer, "Existing Corp");
    }

    #[test]
    fn status_message_ttl() {
        let (_dir, conn) = test_db();
        let mut mgr = SettingsManager::new(&conn, "Hello").unwrap();

        // Save a name to trigger status message
        mgr.handle_key(KeyCode::Enter, &conn);
        for c in "Test".chars() {
            mgr.handle_key(KeyCode::Char(c), &conn);
        }
        mgr.handle_key(KeyCode::Enter, &conn);
        assert!(mgr.status_message.is_some());

        // 3 more keypresses should keep status alive (TTL decrements from 3)
        mgr.handle_key(KeyCode::Down, &conn); // tick 3->2
        assert!(mgr.status_message.is_some());
        mgr.handle_key(KeyCode::Up, &conn); // tick 2->1
        assert!(mgr.status_message.is_some());
        mgr.handle_key(KeyCode::Down, &conn); // tick 1->0, cleared
        assert!(mgr.status_message.is_none());
    }

    #[test]
    fn toggle_update_check() {
        let _config = crate::settings::TempConfigDir::new();
        let (_dir, conn) = test_db();
        let mut mgr = SettingsManager::new(&conn, "Hello").unwrap();

        // Default is enabled
        assert!(mgr.update_check);

        // Navigate to update check menu item
        mgr.selection = MENU_UPDATE_CHECK;

        // Toggle off
        mgr.handle_key(KeyCode::Enter, &conn);
        assert!(!mgr.update_check);

        // Toggle back on
        mgr.handle_key(KeyCode::Enter, &conn);
        assert!(mgr.update_check);
    }

    #[test]
    fn update_check_loads_from_settings() {
        let _config = crate::settings::TempConfigDir::new();
        let (_dir, conn) = test_db();
        let mgr = SettingsManager::new(&conn, "Hello").unwrap();
        // update_check defaults to true from settings
        assert!(mgr.update_check);
    }
}
