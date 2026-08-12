use crossterm::event::KeyCode;
use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};
use rusqlite::Connection;

use crate::cli::invoice::{
    build_clients, company_name, contact_email_for_preview, optional_gateway, optional_publisher,
    PUBLISHED_VOID_NOTICE,
};
use crate::error::{NigelError, Result};
use crate::fmt::money;
use crate::invoicing::clients::{get_client, list_clients, ClientScope};
use crate::invoicing::gateway::{AssetPublisher, Mailer, PaymentGateway};
use crate::invoicing::invoices::{
    create_invoice, ensure_not_void, ensure_voidable, get_invoice, is_void, line_items,
    list_invoices, paid_amount, payment_amount, payments, record_payment, validate_currency,
    validate_date, validate_items, InvoiceListRow, NewLineItem,
};
use crate::invoicing::render_html::{load_template, Branding};
use crate::invoicing::send::send_invoice;
use crate::invoicing::void::{has_teardown_work, void_invoice_with_teardown};
use crate::models::{Client, Invoice, InvoiceLineItem, InvoicePayment};
use crate::settings::{get_data_dir, invoicing_config, InvoicingConfig};
use crate::tui::{FOOTER_STYLE, GREEN, HEADER_STYLE};

pub enum InvoiceAction {
    Continue,
    Close,
    /// The screen has entered a blocking state and needs the controller to
    /// paint it before the work runs.
    Perform,
}

enum Screen {
    List,
    NewInvoice(InvoiceForm),
    Detail,
    PayForm(PayForm),
    ConfirmVoid,
    ConfirmSend,
    Sending,
    /// Void, once it has been confirmed. Like `Sending`, this is a frame the
    /// controller paints before the work runs: voiding a published invoice
    /// deactivates its Stripe payment link and republishes its page, and those
    /// are network calls on the same thread that reads keys.
    Voiding,
    ActionResult {
        title: String,
        lines: Vec<String>,
        is_error: bool,
    },
}

/// The four methods `invoice_payments.method` allows. A fifth option would be
/// a CHECK-constraint failure at insert time, not a compile error.
const METHODS: &[&str] = &["direct_deposit", "ach", "stripe", "other"];

struct PayForm {
    amount: String,
    date: String,
    method: usize,
    focused: usize,
}

impl PayForm {
    /// Prefilled with the outstanding balance and today; a settled invoice
    /// opens with an empty amount rather than a zero to delete.
    fn new(balance: f64, today: &str) -> Self {
        Self {
            amount: if balance < 0.005 {
                String::new()
            } else {
                format!("{balance:.2}")
            },
            date: today.to_string(),
            method: 0,
            focused: 0,
        }
    }

    fn method(&self) -> &'static str {
        METHODS[self.method]
    }

    fn push(&mut self, c: char) {
        match self.focused {
            AMOUNT_IDX if c.is_ascii_digit() || c == '.' || c == ',' => self.amount.push(c),
            DATE_IDX if c.is_ascii_digit() || c == '-' => self.date.push(c),
            _ => {}
        }
    }

    fn backspace(&mut self) {
        match self.focused {
            AMOUNT_IDX => {
                self.amount.pop();
            }
            DATE_IDX => {
                self.date.pop();
            }
            _ => {}
        }
    }
}

const AMOUNT_IDX: usize = 0;
const DATE_IDX: usize = 1;
const METHOD_IDX: usize = 2;
const PAY_FIELDS: usize = 3;

/// The draft form's four header fields, in the order `create_invoice` takes
/// them. The line-item cells continue the same focus index past
/// `HEADER_FIELDS`.
const CLIENT_IDX: usize = 0;
const ISSUE_IDX: usize = 1;
const DUE_IDX: usize = 2;
const CURRENCY_IDX: usize = 3;
const HEADER_FIELDS: usize = 4;

const DESC_CELL: usize = 0;
const QTY_CELL: usize = 1;
const UNIT_CELL: usize = 2;
const CELLS_PER_ROW: usize = 3;

/// The due date a fresh form suggests. `nigel invoice new` has no default; a
/// form has a field that must hold something, and Net 30 is what the stock
/// invoice template's terms say.
const DEFAULT_TERM_DAYS: i64 = 30;

/// One line-item row mid-edit. Every cell is text until Enter, because a
/// half-typed quantity is a state the form has to be able to hold.
#[derive(Default)]
struct ItemRow {
    description: String,
    quantity: String,
    unit_amount: String,
}

impl ItemRow {
    /// The row's amount, once both figures parse — `None` while either is
    /// blank or half-typed, which renders as an em dash rather than an
    /// invented `0.00`.
    fn amount(&self) -> Option<f64> {
        Some(cell_number(&self.quantity).ok()? * cell_number(&self.unit_amount).ok()?)
    }
}

/// The two rules a form owns that the data layer cannot see: a blank cell and
/// an unparseable one. `cli::invoice`'s `--item` parser reports both against
/// the flag (`bad quantity '2x' in --item '…'`); beside a field, the field is
/// what needs naming — the same call `record_pay_form` makes about `--amount`.
fn cell_number(value: &str) -> std::result::Result<f64, &'static str> {
    let trimmed = value.trim().replace(',', "");
    if trimmed.is_empty() {
        return Err("is required");
    }
    trimmed.parse().map_err(|_| "must be a number")
}

fn plus_days(date: &str, days: i64) -> Option<String> {
    let parsed = chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d").ok()?;
    Some(
        parsed
            .checked_add_signed(chrono::Duration::days(days))?
            .format("%Y-%m-%d")
            .to_string(),
    )
}

/// A draft invoice being typed. A refusal is held as the field it belongs to
/// plus the sentence the data layer wrote, so it can be rendered beside that
/// field rather than in a status line at the bottom.
struct InvoiceForm {
    /// Never empty: the screen refuses to open the form on a book with no
    /// clients, so `client()` always has a row to point at.
    clients: Vec<Client>,
    client_idx: usize,
    issue_date: String,
    due_date: String,
    currency: String,
    items: Vec<ItemRow>,
    focused: usize,
    error: Option<(usize, String)>,
}

impl InvoiceForm {
    fn new(clients: Vec<Client>, today: &str) -> Self {
        Self {
            clients,
            client_idx: 0,
            issue_date: today.to_string(),
            due_date: plus_days(today, DEFAULT_TERM_DAYS).unwrap_or_default(),
            currency: "USD".to_string(),
            items: vec![ItemRow::default()],
            focused: CLIENT_IDX,
            error: None,
        }
    }

    fn client(&self) -> &Client {
        &self.clients[self.client_idx]
    }

    fn field_count(&self) -> usize {
        HEADER_FIELDS + CELLS_PER_ROW * self.items.len()
    }

    fn cell_index(row: usize, cell: usize) -> usize {
        HEADER_FIELDS + row * CELLS_PER_ROW + cell
    }

    /// The row and cell the focus is in, or `None` on a header field.
    fn focused_cell(&self) -> Option<(usize, usize)> {
        let offset = self.focused.checked_sub(HEADER_FIELDS)?;
        Some((offset / CELLS_PER_ROW, offset % CELLS_PER_ROW))
    }

    /// What the rows that currently parse add up to.
    fn total(&self) -> f64 {
        self.items.iter().filter_map(ItemRow::amount).sum()
    }

    fn push(&mut self, c: char) {
        self.error = None;
        match self.focused_cell() {
            Some((row, DESC_CELL)) if !c.is_control() => self.items[row].description.push(c),
            Some((row, QTY_CELL)) if c.is_ascii_digit() || c == '.' => {
                self.items[row].quantity.push(c)
            }
            Some((row, UNIT_CELL)) if c.is_ascii_digit() || c == '.' || c == ',' => {
                self.items[row].unit_amount.push(c)
            }
            Some(_) => {}
            None => match self.focused {
                ISSUE_IDX if c.is_ascii_digit() || c == '-' => self.issue_date.push(c),
                DUE_IDX if c.is_ascii_digit() || c == '-' => self.due_date.push(c),
                // Uppercased on the way in, because that is the only shape
                // `validate_currency` stores.
                CURRENCY_IDX if c.is_ascii_alphabetic() => {
                    self.currency.push(c.to_ascii_uppercase())
                }
                _ => {}
            },
        }
    }

    fn backspace(&mut self) {
        self.error = None;
        let field = match self.focused_cell() {
            Some((row, DESC_CELL)) => &mut self.items[row].description,
            Some((row, QTY_CELL)) => &mut self.items[row].quantity,
            Some((row, UNIT_CELL)) => &mut self.items[row].unit_amount,
            Some(_) => return,
            None => match self.focused {
                ISSUE_IDX => &mut self.issue_date,
                DUE_IDX => &mut self.due_date,
                CURRENCY_IDX => &mut self.currency,
                _ => return,
            },
        };
        field.pop();
    }

    /// A new row below the focused one — or at the end, when the focus is on a
    /// header field — taking the focus with it.
    fn add_row(&mut self) {
        let at = self
            .focused_cell()
            .map_or(self.items.len(), |(row, _)| row + 1);
        self.items.insert(at, ItemRow::default());
        self.focused = Self::cell_index(at, DESC_CELL);
        self.error = None;
    }

    fn remove_row(&mut self) {
        let Some((row, _)) = self.focused_cell() else {
            return;
        };
        if self.items.len() == 1 {
            // Asking the data layer rather than restating it: an invoice with
            // no lines is exactly what `validate_items` refuses on Enter.
            let message = validate_items(&[])
                .expect_err("an empty item list is refused")
                .to_string();
            self.error = Some((Self::cell_index(0, DESC_CELL), message));
            return;
        }
        self.items.remove(row);
        self.focused = Self::cell_index(row.min(self.items.len() - 1), DESC_CELL);
        self.error = None;
    }

    fn line_items(&self) -> std::result::Result<Vec<NewLineItem>, (usize, String)> {
        self.items
            .iter()
            .enumerate()
            .map(|(row, item)| {
                Ok(NewLineItem {
                    description: item.description.trim().to_string(),
                    quantity: cell_number(&item.quantity).map_err(|why| {
                        (Self::cell_index(row, QTY_CELL), format!("Quantity {why}"))
                    })?,
                    unit_amount: cell_number(&item.unit_amount).map_err(|why| {
                        (
                            Self::cell_index(row, UNIT_CELL),
                            format!("Unit amount {why}"),
                        )
                    })?,
                })
            })
            .collect()
    }

    /// Create the draft, or answer with the field a refusal belongs to.
    ///
    /// The three validators run here are the ones `create_invoice` runs, in its
    /// order, and they run **only to attribute a failure to a field**;
    /// `create_invoice` re-runs every one of them and stays the sole writer, so
    /// the screen never becomes the authority on what a valid invoice is.
    /// Whatever it refuses that was not attributed above — a client deleted
    /// from another screen, a database failure — lands on the client field,
    /// which is the only one that could have caused it.
    fn submit(&self, conn: &Connection) -> std::result::Result<i64, (usize, String)> {
        let items = self.line_items()?;
        let issue = self.issue_date.trim();
        let due = self.due_date.trim();
        let currency = self.currency.trim();

        validate_items(&items).map_err(|e| (Self::cell_index(0, DESC_CELL), e.to_string()))?;
        validate_date(issue, "issue").map_err(|e| (ISSUE_IDX, e.to_string()))?;
        if !due.is_empty() {
            validate_date(due, "due").map_err(|e| (DUE_IDX, e.to_string()))?;
        }
        validate_currency(currency).map_err(|e| (CURRENCY_IDX, e.to_string()))?;

        create_invoice(
            conn,
            self.client().id,
            issue,
            (!due.is_empty()).then_some(due),
            currency,
            &items,
            None,
            None,
        )
        .map_err(|e| (CLIENT_IDX, e.to_string()))
    }
}

/// Everything the detail view shows, loaded on entry and reloaded after every
/// mutation.
struct Detail {
    invoice: Invoice,
    /// `None` when the invoice's client row is gone. Nothing in Nigel creates
    /// that state — `clients::delete_client` refuses a client with invoices of
    /// any status — but the schema represents it, and the screen that would
    /// have refused to open is the one place it would be noticed.
    client: Option<Client>,
    items: Vec<InvoiceLineItem>,
    payments: Vec<InvoicePayment>,
    paid: f64,
}

impl Detail {
    fn load(conn: &Connection, invoice_id: i64) -> Result<Self> {
        let invoice = get_invoice(conn, invoice_id)?;
        let client = match get_client(conn, invoice.client_id) {
            Ok(client) => Some(client),
            // Only a missing row is survivable; a database failure still is not.
            Err(NigelError::NotFound(_)) => None,
            Err(e) => return Err(e),
        };
        Ok(Self {
            items: line_items(conn, invoice.id)?,
            payments: payments(conn, invoice.id)?,
            paid: paid_amount(conn, invoice.id)?,
            invoice,
            client,
        })
    }

    fn balance(&self) -> f64 {
        self.invoice.total - self.paid
    }

    /// The client's name, or an em dash — the same treatment every other
    /// optional field on this screen gets.
    fn client_name(&self) -> String {
        optional_display(self.client.as_ref().map(|c| c.name.as_str()))
    }

    fn client_email(&self) -> Option<&str> {
        self.client.as_ref()?.email.as_deref()
    }
}

pub struct InvoiceManager {
    rows: Vec<InvoiceListRow>,
    selection: usize,
    scroll_offset: usize,
    last_visible_rows: usize,
    screen: Screen,
    detail: Option<Box<Detail>>,
    detail_scroll: usize,
    status_message: Option<String>,
    /// Remaining keypresses before the status message is cleared.
    status_ttl: u8,
    greeting: String,
}

impl InvoiceManager {
    pub fn new(conn: &Connection, greeting: &str) -> Self {
        Self {
            rows: list_invoices(conn, None, None).unwrap_or_default(),
            selection: 0,
            scroll_offset: 0,
            last_visible_rows: 20,
            screen: Screen::List,
            detail: None,
            detail_scroll: 0,
            status_message: None,
            status_ttl: 0,
            greeting: greeting.to_string(),
        }
    }

    fn reload_list(&mut self, conn: &Connection) {
        self.rows = list_invoices(conn, None, None).unwrap_or_default();
        if self.rows.is_empty() {
            self.selection = 0;
        } else {
            self.selection = self.selection.min(self.rows.len() - 1);
        }
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

    /// Load (or reload) the detail for one invoice.
    fn load_detail(&mut self, conn: &Connection, invoice_id: i64) -> Result<()> {
        self.detail = Some(Box::new(Detail::load(conn, invoice_id)?));
        Ok(())
    }

    fn selected_id(&self) -> Option<i64> {
        self.rows.get(self.selection).map(|r| r.id)
    }

    pub fn draw(&mut self, frame: &mut Frame) {
        match &self.screen {
            Screen::List => self.draw_list(frame),
            Screen::NewInvoice(form) => self.draw_new_form(frame, form),
            Screen::Detail | Screen::ConfirmVoid | Screen::ConfirmSend => self.draw_detail(frame),
            Screen::PayForm(form) => self.draw_pay_form(frame, form),
            Screen::Sending => self.draw_sending(frame),
            Screen::Voiding => self.draw_voiding(frame),
            Screen::ActionResult {
                title,
                lines,
                is_error,
            } => self.draw_action_result(frame, title, lines, *is_error),
        }
    }

    fn draw_action_result(&self, frame: &mut Frame, title: &str, body: &[String], is_error: bool) {
        let (content_area, hints_area) = self.draw_chrome(frame);
        let title_style = if is_error {
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(GREEN).add_modifier(Modifier::BOLD)
        };

        let mut lines = vec![
            Line::from(""),
            Line::from(Span::styled(format!(" {title}"), title_style)),
            Line::from(""),
        ];
        // Wrapped, because what lands here is a sentence from somewhere else —
        // an upstream's refusal, or a payment link an operator has to open. A
        // truncated URL is a URL nobody can use.
        for line in body {
            let (wrapped, _) =
                crate::tui::wrap_text(line, (content_area.width as usize).max(20) - 6);
            for wrapped_line in wrapped.lines() {
                lines.push(Line::from(format!("   {wrapped_line}")));
            }
        }
        frame.render_widget(Paragraph::new(lines), content_area);
        frame.render_widget(Paragraph::new(" Esc=back").style(FOOTER_STYLE), hints_area);
    }

    /// S7. The terminal really is unresponsive for the duration of the send,
    /// so the frame says so rather than animating a spinner it cannot advance.
    fn draw_sending(&self, frame: &mut Frame) {
        let (content_area, hints_area) = self.draw_chrome(frame);
        let Some(detail) = &self.detail else {
            return;
        };
        let email = optional_display(detail.client_email());

        let lines = vec![
            Line::from(""),
            Line::from(Span::styled(
                format!(" Sending invoice #{}", detail.invoice.number),
                Style::default().add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from("   Creating the Stripe payment link, publishing the page and PDF, and"),
            Line::from(format!("   emailing {email}.")),
            Line::from(""),
            Line::from("   This can take a few seconds. Nigel is not reading keys until it"),
            Line::from("   finishes."),
        ];
        frame.render_widget(Paragraph::new(lines), content_area);
        frame.render_widget(
            Paragraph::new(" Working\u{2026}").style(FOOTER_STYLE),
            hints_area,
        );
    }

    /// The draft form. Header fields, then the line-item table, then the
    /// running total — with any refusal on its own line directly under the
    /// field it is about.
    fn draw_new_form(&self, frame: &mut Frame, form: &InvoiceForm) {
        let (content_area, hints_area) = self.draw_chrome(frame);

        let mut lines = vec![
            Line::from(""),
            Line::from(Span::styled(
                " New Draft Invoice",
                Style::default().add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
        ];

        // Which rendered line the focus is on, so a form taller than the
        // terminal scrolls to follow it — the arrow keys move between fields
        // here, so there is no scroll key left to press.
        let mut focus_line = 0;
        let client = form.client().name.clone();
        for (idx, label, value) in [
            (
                CLIENT_IDX,
                "Client",
                if form.focused == CLIENT_IDX {
                    format!("< {client} >")
                } else {
                    format!("  {client}")
                },
            ),
            (ISSUE_IDX, "Issue date", format!("  {}", form.issue_date)),
            (DUE_IDX, "Due date", format!("  {}", form.due_date)),
            (CURRENCY_IDX, "Currency", format!("  {}", form.currency)),
        ] {
            let focused = form.focused == idx;
            let (label_style, value_style, cursor) = if focused {
                (
                    Style::default().add_modifier(Modifier::BOLD),
                    Style::default().fg(Color::Cyan),
                    if idx == CLIENT_IDX { "" } else { "_" },
                )
            } else {
                (Style::default(), Style::default(), "")
            };
            // The focus anchor is the *last* line of the focused field, its
            // refusal included — otherwise the message renders one line past
            // the bottom of the window and Enter looks like it did nothing.
            if focused {
                focus_line = lines.len();
            }
            lines.push(Line::from(vec![
                Span::styled(format!("   {label:<12} "), label_style),
                Span::styled(format!("{value}{cursor}"), value_style),
            ]));
            if let Some(message) = error_line(form, idx, FIELD_VALUE_COLUMN) {
                if focused {
                    focus_line = lines.len();
                }
                lines.push(message);
            }
        }

        let (table, columns) = item_table(form);
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            columns.pad(&table[0]).concat(),
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )));
        for row in 0..form.items.len() {
            let row_focused = form.focused_cell().is_some_and(|(r, _)| r == row);
            if row_focused {
                focus_line = lines.len();
            }
            lines.push(item_line(form, row, &columns.pad(&table[row + 1])));
            for cell in [DESC_CELL, QTY_CELL, UNIT_CELL] {
                let anchor = InvoiceForm::cell_index(row, cell);
                if let Some(message) = error_line(form, anchor, ITEM_TEXT_COLUMN) {
                    if row_focused {
                        focus_line = lines.len();
                    }
                    lines.push(message);
                }
            }
        }

        lines.push(Line::from(""));
        // Through the table's own budget, so the running total lands under the
        // Amount column rather than under the detail view's narrower one.
        lines.push(Line::from(columns.pad(&table[table.len() - 1]).concat()));
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            format!("   {DRAFT_HINT}"),
            FOOTER_STYLE,
        )));

        let visible = (content_area.height as usize).max(1);
        let start = (focus_line + 1).saturating_sub(visible);
        let end = (start + visible).min(lines.len());
        frame.render_widget(Paragraph::new(lines[start..end].to_vec()), content_area);

        // Left/Right does nothing anywhere but the selector, so only the
        // selector advertises it.
        let hint = match form.focused {
            CLIENT_IDX => {
                " Tab=field  Left/Right=client  Ins/F2=add line  Enter=create  Esc=cancel"
            }
            _ if form.focused_cell().is_some() => {
                " Tab=field  Ins/F2=add line  Del/F3=remove line  Enter=create  Esc=cancel"
            }
            _ => " Tab=field  Ins/F2=add line  Enter=create  Esc=cancel",
        };
        frame.render_widget(Paragraph::new(hint).style(FOOTER_STYLE), hints_area);
    }

    /// The void counterpart of S7, and only ever painted for an invoice with
    /// something live behind it: `begin_void` runs a teardown-free void inline
    /// and never enters this state, so the two calls this frame names are calls
    /// that are actually about to be made.
    fn draw_voiding(&self, frame: &mut Frame) {
        let (content_area, hints_area) = self.draw_chrome(frame);
        let Some(detail) = &self.detail else {
            return;
        };

        let lines = vec![
            Line::from(""),
            Line::from(Span::styled(
                format!(" Voiding invoice #{}", detail.invoice.number),
                Style::default().add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from("   Cancelling it, deactivating its Stripe payment link and replacing"),
            Line::from("   its published page."),
            Line::from(""),
            Line::from("   This can take a few seconds. Nigel is not reading keys until it"),
            Line::from("   finishes."),
        ];
        frame.render_widget(Paragraph::new(lines), content_area);
        frame.render_widget(
            Paragraph::new(" Working\u{2026}").style(FOOTER_STYLE),
            hints_area,
        );
    }

    fn draw_pay_form(&self, frame: &mut Frame, form: &PayForm) {
        let (content_area, hints_area) = self.draw_chrome(frame);
        let Some(detail) = &self.detail else {
            return;
        };

        let mut lines = vec![
            Line::from(""),
            Line::from(Span::styled(
                format!(
                    " Record a Payment \u{2014} invoice #{}",
                    detail.invoice.number
                ),
                Style::default().add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(format!("   Client     {}", detail.client_name())),
            Line::from(format!(
                "   Total      {:<14} Paid  {:<14} Balance  {}",
                money(detail.invoice.total),
                money(detail.paid),
                money(detail.balance()),
            )),
            Line::from(""),
        ];

        for (idx, label, value) in [
            (AMOUNT_IDX, "Amount", format!("$ {}", form.amount)),
            (DATE_IDX, "Date", format!("  {}", form.date)),
            (
                METHOD_IDX,
                "Method",
                if form.focused == METHOD_IDX {
                    format!("< {} >", form.method())
                } else {
                    format!("  {}  ", form.method())
                },
            ),
        ] {
            let focused = form.focused == idx;
            let (label_style, value_style, cursor) = if focused {
                (
                    Style::default().add_modifier(Modifier::BOLD),
                    Style::default().fg(Color::Cyan),
                    if idx == METHOD_IDX { "" } else { "_" },
                )
            } else {
                (Style::default(), Style::default(), "")
            };
            lines.push(Line::from(vec![
                Span::styled(format!("   {label:<10} "), label_style),
                Span::styled(format!("{value}{cursor}"), value_style),
            ]));
        }

        if let Some(msg) = &self.status_message {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                format!("   {msg}"),
                Style::default().fg(Color::Yellow),
            )));
        }

        frame.render_widget(Paragraph::new(lines), content_area);
        frame.render_widget(
            Paragraph::new(" Tab=next field  Left/Right=method  Enter=record  Esc=cancel")
                .style(FOOTER_STYLE),
            hints_area,
        );
    }

    fn draw_detail(&mut self, frame: &mut Frame) {
        let (content_area, hints_area) = self.draw_chrome(frame);
        let Some(detail) = self.detail.as_deref() else {
            return;
        };
        let invoice = &detail.invoice;

        let mut lines = vec![
            Line::from(""),
            Line::from(vec![
                Span::styled(
                    format!(" Invoice #{}   ", invoice.number),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::styled(invoice.status.clone(), status_style(&invoice.status)),
            ]),
            Line::from(""),
            Line::from(format!("   Client    {}", detail.client_name())),
            Line::from(format!(
                "   Email     {}",
                optional_display(detail.client_email())
            )),
            Line::from(format!(
                "   Issued    {:<16} Due  {:<16} Currency  {}",
                invoice.issue_date,
                optional_display(invoice.due_date.as_deref()),
                invoice.currency,
            )),
        ];
        if let Some(voided_at) = &invoice.voided_at {
            lines.push(Line::from(format!("   Voided    {voided_at}")));
            for line in warning_lines(invoice, content_area.width) {
                lines.push(Line::from(Span::styled(
                    line,
                    Style::default().fg(Color::Yellow),
                )));
            }
        }

        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            format!(
                "   {:<40} {:>8} {:>11} {:>12}",
                "Description", "Qty", "Unit", "Amount"
            ),
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )));
        for item in &detail.items {
            lines.push(Line::from(format!(
                "   {:<40} {:>8.2} {:>11.2} {:>12.2}",
                truncate(&item.description, 38),
                item.quantity,
                item.unit_amount,
                item.line_total,
            )));
        }
        lines.push(total_line("Subtotal", invoice.subtotal));
        lines.push(total_line("Tax", invoice.tax));
        lines.push(total_line("Total", invoice.total));

        // No empty table: the section only exists once there is a payment.
        if !detail.payments.is_empty() {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "   Payments",
                Style::default().add_modifier(Modifier::BOLD),
            )));
            for payment in &detail.payments {
                lines.push(Line::from(format!(
                    "   {}   {:<20} {:>26}",
                    payment.paid_date,
                    payment.method,
                    money(payment.amount)
                )));
            }
        }
        lines.push(total_line("Paid", detail.paid));
        lines.push(total_line("Balance", detail.balance()));

        if let Some(url) = &invoice.stripe_payment_link_url {
            lines.push(Line::from(""));
            lines.push(Line::from(format!("   Pay link  {url}")));
        }

        if let Screen::ConfirmSend = &self.screen {
            lines.push(Line::from(""));
            let cfg = crate::settings::invoicing_config();
            let base_url_warning = cfg
                .public_base_url
                .as_deref()
                .filter(|url| crate::invoicing::r2::validate_public_base_url(url).is_ok())
                .and_then(crate::invoicing::r2::public_base_url_warning);
            for line in send_confirmation(detail, base_url_warning, content_area.width) {
                lines.push(Line::from(Span::styled(
                    format!("   {line}"),
                    Style::default().fg(Color::Yellow),
                )));
            }
        }

        // The invoice stays on screen while the confirmation is answered.
        if let Screen::ConfirmVoid = &self.screen {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                format!(
                    "   Void invoice #{} for {} ({})?",
                    invoice.number,
                    detail.client_name(),
                    money(invoice.total)
                ),
                Style::default().fg(Color::Yellow),
            )));
            lines.push(Line::from(Span::styled(
                "   Void is permanent. A void invoice can never be sent or paid.",
                Style::default().fg(Color::Yellow),
            )));
            for line in warning_lines(invoice, content_area.width) {
                lines.push(Line::from(Span::styled(
                    line,
                    Style::default().fg(Color::Yellow),
                )));
            }
        }

        // Clamped here rather than on the key, because how far this scrolls
        // depends on how tall the terminal is.
        let visible = (content_area.height as usize).max(1);
        let start = self.detail_scroll.min(lines.len().saturating_sub(visible));
        let end = (start + visible).min(lines.len());
        frame.render_widget(Paragraph::new(lines[start..end].to_vec()), content_area);
        self.detail_scroll = start;

        if let Some(msg) = &self.status_message {
            frame.render_widget(
                Paragraph::new(format!(" {msg}")).style(Style::default().fg(Color::Yellow)),
                hints_area,
            );
        } else if let Screen::ConfirmVoid = &self.screen {
            frame.render_widget(
                Paragraph::new(" y=void  n=cancel").style(FOOTER_STYLE),
                hints_area,
            );
        } else if let Screen::ConfirmSend = &self.screen {
            frame.render_widget(
                Paragraph::new(" y=send  n=cancel").style(FOOTER_STYLE),
                hints_area,
            );
        } else {
            let hint = if is_void(invoice) {
                " Up/Down=scroll  Esc=back  q=quit"
            } else {
                " s=send  p=record payment  v=void  Up/Down=scroll  Esc=back  q=quit"
            };
            frame.render_widget(Paragraph::new(hint).style(FOOTER_STYLE), hints_area);
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

        let mut lines = vec![
            Line::from(""),
            Line::from(Span::styled(
                format!(" Invoices ({})", self.rows.len()),
                Style::default().add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
        ];

        if self.rows.is_empty() {
            lines.push(Line::from("   No invoices yet. Press 'a' to draft one."));
        } else {
            lines.push(Line::from(Span::styled(
                format!(
                    "   {:<6} {:<STATUS_WIDTH$} {:<24} {:>12} {:>12} {}",
                    "#", "Status", "Client", "Total", "Balance", "Due"
                ),
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            )));

            let end = (self.scroll_offset + data_rows).min(self.rows.len());
            for i in self.scroll_offset..end {
                let row = &self.rows[i];
                let marker = if i == self.selection { " > " } else { "   " };
                let base = if i == self.selection {
                    Style::default().add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };
                // Only the status cell carries colour; the figures are all
                // positive receivables, so a sign-derived colour would be noise.
                let (number, status, rest) = list_cells(marker, row);
                lines.push(Line::from(vec![
                    Span::styled(number, base),
                    Span::styled(status, status_style(&row.status).patch(base)),
                    Span::styled(rest, base),
                ]));
            }
        }

        frame.render_widget(Paragraph::new(lines), content_area);

        if let Some(msg) = &self.status_message {
            frame.render_widget(
                Paragraph::new(format!(" {msg}")).style(Style::default().fg(Color::Yellow)),
                hints_area,
            );
        } else {
            frame.render_widget(
                Paragraph::new(" a=new  Enter=open  Esc=back  q=quit").style(FOOTER_STYLE),
                hints_area,
            );
        }
    }

    pub fn handle_key(&mut self, code: KeyCode, conn: &Connection) -> InvoiceAction {
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
            Screen::NewInvoice(_) => self.handle_new_key(code, conn),
            Screen::Detail => self.handle_detail_key(code, conn),
            Screen::PayForm(_) => self.handle_pay_key(code, conn),
            Screen::ConfirmVoid => self.handle_void_key(code, conn),
            Screen::ConfirmSend => self.handle_confirm_send_key(code),
            // Any key dismisses the result and returns to the reloaded detail.
            Screen::ActionResult { .. } => {
                self.screen = Screen::Detail;
                InvoiceAction::Continue
            }
            // The screen is painted and then blocks; no key is read until the
            // work returns.
            Screen::Sending | Screen::Voiding => InvoiceAction::Continue,
        }
    }

    /// `s` on the detail view. Every guard runs before the dialog opens, so it
    /// never offers something that is going to fail: the void check, the
    /// client's email, the invoice template, and the invoicing config — in the
    /// order `nigel invoice send` runs them, and none of them touches the
    /// network.
    pub(crate) fn begin_send(
        &mut self,
        conn: &Connection,
        cfg: InvoicingConfig,
        data_dir: &std::path::Path,
    ) -> InvoiceAction {
        let Some(detail) = &self.detail else {
            return InvoiceAction::Continue;
        };
        if let Err(e) = ensure_not_void(&detail.invoice, "sent") {
            self.set_status(e.to_string());
            return InvoiceAction::Continue;
        }
        if detail.client_email().is_none() {
            // send.rs's own wording, so the two front ends cannot disagree.
            let name = detail.client_name();
            self.set_status(format!("client '{name}' has no email"));
            return InvoiceAction::Continue;
        }
        if let Err(e) = load_template(data_dir) {
            self.set_status(e.to_string());
            return InvoiceAction::Continue;
        }
        if let Err(e) = build_clients(cfg, &company_name(conn)) {
            self.set_status(e.to_string());
            return InvoiceAction::Continue;
        }
        self.open_confirmation(Screen::ConfirmSend);
        InvoiceAction::Continue
    }

    /// A confirmation is rendered at the bottom of the detail view and answered
    /// from the footer, so it needs both of those: the question scrolled into
    /// view, and a footer showing y/n rather than a status line left over from
    /// the last keypress.
    fn open_confirmation(&mut self, screen: Screen) {
        self.screen = screen;
        self.status_message = None;
        self.status_ttl = 0;
        self.detail_scroll = usize::MAX;
    }

    /// Run the work the confirmation authorised. The controller calls this
    /// after painting S7 or its void counterpart, because the work blocks the
    /// whole loop.
    pub fn perform_pending(&mut self, conn: &Connection) {
        self.perform_pending_with(
            conn,
            &crate::cli::today(),
            invoicing_config(),
            &get_data_dir(),
        );
        drain_buffered_input();
    }

    /// The screen is what says which work is pending: both blocking states are
    /// entered by the key handler that returned `Perform`, so the two cannot get
    /// out of step with a flag kept beside them.
    pub(crate) fn perform_pending_with(
        &mut self,
        conn: &Connection,
        today: &str,
        cfg: InvoicingConfig,
        data_dir: &std::path::Path,
    ) {
        if self.detail.is_none() {
            return;
        }
        match self.screen {
            Screen::Voiding => self.perform_pending_void(conn, today, cfg),
            _ => self.perform_pending_send(conn, today, cfg, data_dir),
        }
    }

    fn perform_pending_send(
        &mut self,
        conn: &Connection,
        today: &str,
        cfg: InvoicingConfig,
        data_dir: &std::path::Path,
    ) {
        let company = company_name(conn);
        let contact_email = contact_email_for_preview(&cfg).0;
        let prepared = load_template(data_dir).and_then(|template| {
            let clients = build_clients(cfg, &company)?;
            Ok((template, clients))
        });
        match prepared {
            Ok((template, clients)) => {
                // The one place a send's config warnings are surfaced on this
                // screen: `begin_send` builds clients too, and printing there
                // as well would say it twice. Never `eprintln!` — ratatui owns
                // the alternate screen this is drawn into.
                for warning in &clients.warnings {
                    self.set_status(warning.clone());
                }
                let branding = Branding {
                    template: &template,
                    company: &company,
                    contact_email: &contact_email,
                };
                self.perform_send(
                    conn,
                    today,
                    &branding,
                    &clients.stripe,
                    &clients.r2,
                    &clients.mail,
                );
            }
            Err(e) => self.finish_send(conn, Err(e)),
        }
    }

    /// Void needs no template and refuses nothing for want of a key: it takes
    /// whichever clients this installation has and reports what it could not
    /// reach.
    fn perform_pending_void(&mut self, conn: &Connection, today: &str, cfg: InvoicingConfig) {
        self.perform_void(
            conn,
            today,
            optional_gateway(&cfg).as_ref(),
            optional_publisher(&cfg).as_ref(),
        );
    }

    /// The testable half: the same orchestration against injected clients.
    pub(crate) fn perform_send<G: PaymentGateway, P: AssetPublisher, M: Mailer>(
        &mut self,
        conn: &Connection,
        today: &str,
        branding: &Branding<'_>,
        gateway: &G,
        publisher: &P,
        mailer: &M,
    ) {
        let Some(detail) = &self.detail else {
            return;
        };
        let outcome = send_invoice(
            conn,
            detail.invoice.id,
            today,
            branding,
            gateway,
            publisher,
            mailer,
        );
        self.finish_send(conn, outcome);
    }

    fn finish_send(&mut self, conn: &Connection, outcome: Result<String>) {
        let Some(detail) = &self.detail else {
            return;
        };
        let (invoice_id, number) = (detail.invoice.id, detail.invoice.number);
        self.reload_list(conn);
        if let Err(e) = self.load_detail(conn, invoice_id) {
            self.detail = None;
            self.screen = Screen::List;
            self.set_status(e.to_string());
            return;
        }
        let reloaded = self.detail.as_ref().expect("just loaded");

        let (title, lines, is_error) = match outcome {
            Ok(url) => (
                format!("Invoice #{number} sent"),
                vec![
                    url,
                    format!("Emailed to {}.", optional_display(reloaded.client_email())),
                ],
                false,
            ),
            // The second sentence is derived from the reloaded row: a failed
            // first send leaves a draft, a failed re-send leaves an invoice
            // that is still published.
            Err(e) => (
                "Send failed".to_string(),
                vec![
                    e.to_string(),
                    format!(
                        "Invoice #{number} is still {}. Nothing was published or emailed.",
                        reloaded.invoice.status
                    ),
                ],
                true,
            ),
        };
        self.screen = Screen::ActionResult {
            title,
            lines,
            is_error,
        };
    }

    fn handle_confirm_send_key(&mut self, code: KeyCode) -> InvoiceAction {
        match code {
            KeyCode::Char('y') => {
                self.screen = Screen::Sending;
                // The controller paints S7 before running the send, so the
                // frozen frame is the one that says it is frozen.
                InvoiceAction::Perform
            }
            KeyCode::Char('n') | KeyCode::Esc => {
                self.screen = Screen::Detail;
                InvoiceAction::Continue
            }
            _ => InvoiceAction::Continue,
        }
    }

    /// `v` on the detail view, pre-flighted through the data layer's own guard
    /// so the dialog is never offered for an invoice that would refuse it.
    fn open_void_confirmation(&mut self, conn: &Connection) {
        let Some(detail) = &self.detail else {
            return;
        };
        match ensure_voidable(conn, &detail.invoice) {
            Ok(()) => self.open_confirmation(Screen::ConfirmVoid),
            Err(e) => self.set_status(e.to_string()),
        }
    }

    fn handle_void_key(&mut self, code: KeyCode, conn: &Connection) -> InvoiceAction {
        match code {
            KeyCode::Char('y') => self.begin_void(conn),
            KeyCode::Char('n') | KeyCode::Esc => {
                self.screen = Screen::Detail;
                InvoiceAction::Continue
            }
            _ => InvoiceAction::Continue,
        }
    }

    /// Run the void the confirmation authorised, or hand it to the controller.
    ///
    /// An invoice with nothing published and no payment link reaches no network
    /// whatever is configured, so it runs here: it is the database write it
    /// always was, and freezing the terminal behind a frame promising two calls
    /// it will not make would be a lie told at the user's expense. Anything with
    /// something live goes the long way round, like send.
    fn begin_void(&mut self, conn: &Connection) -> InvoiceAction {
        let Some(detail) = &self.detail else {
            return InvoiceAction::Continue;
        };
        if !has_teardown_work(&detail.invoice) {
            self.perform_void(
                conn,
                &crate::cli::today(),
                None::<&crate::invoicing::stripe::StripeClient>,
                None::<&crate::invoicing::r2::R2Publisher>,
            );
            return InvoiceAction::Continue;
        }
        self.screen = Screen::Voiding;
        InvoiceAction::Perform
    }

    /// The testable half of void: the same orchestration against injected
    /// clients, either of which may be absent on an installation that has not
    /// configured it.
    pub(crate) fn perform_void<G: PaymentGateway, P: AssetPublisher>(
        &mut self,
        conn: &Connection,
        today: &str,
        gateway: Option<&G>,
        publisher: Option<&P>,
    ) {
        let Some(detail) = &self.detail else {
            return;
        };
        let (invoice_id, number) = (detail.invoice.id, detail.invoice.number);
        // The date is the fact refresh_status derives `void` from, so it is
        // today's, never an empty string.
        match void_invoice_with_teardown(conn, invoice_id, today, gateway, publisher) {
            Ok(outcome) => {
                self.after_mutation(conn, invoice_id);
                let warnings = outcome.warnings();
                if warnings.is_empty() {
                    self.set_status(format!("Voided invoice #{number}."));
                    return;
                }
                // A live payment link and its URL do not fit on the status line,
                // and truncating the one address an operator has to open would
                // be worse than a screen they dismiss with Esc.
                self.screen = Screen::ActionResult {
                    title: format!("Voided invoice #{number}"),
                    lines: warnings,
                    is_error: false,
                };
            }
            Err(e) => {
                self.screen = Screen::Detail;
                self.set_status(e.to_string());
            }
        }
    }

    /// `p` on the detail view: refused outright for a void invoice, in
    /// `cli::invoice`'s own words, before the form is ever offered.
    fn open_pay_form(&mut self, today: &str) {
        let Some(detail) = &self.detail else {
            return;
        };
        if let Err(e) = ensure_not_void(&detail.invoice, "paid") {
            self.set_status(e.to_string());
            return;
        }
        self.screen = Screen::PayForm(PayForm::new(detail.balance(), today));
    }

    fn handle_pay_key(&mut self, code: KeyCode, conn: &Connection) -> InvoiceAction {
        let Screen::PayForm(form) = &mut self.screen else {
            return InvoiceAction::Continue;
        };
        match code {
            KeyCode::Esc => self.screen = Screen::Detail,
            KeyCode::Tab | KeyCode::Down => form.focused = (form.focused + 1) % PAY_FIELDS,
            KeyCode::BackTab | KeyCode::Up => {
                form.focused = if form.focused == 0 {
                    PAY_FIELDS - 1
                } else {
                    form.focused - 1
                };
            }
            KeyCode::Left => {
                if form.focused == METHOD_IDX {
                    form.method = if form.method == 0 {
                        METHODS.len() - 1
                    } else {
                        form.method - 1
                    };
                }
            }
            KeyCode::Right => {
                if form.focused == METHOD_IDX {
                    form.method = (form.method + 1) % METHODS.len();
                }
            }
            KeyCode::Char(c) => form.push(c),
            KeyCode::Backspace => form.backspace(),
            KeyCode::Enter => self.record_pay_form(conn),
            _ => {}
        }
        InvoiceAction::Continue
    }

    fn record_pay_form(&mut self, conn: &Connection) {
        let (Screen::PayForm(form), Some(detail)) = (&self.screen, &self.detail) else {
            return;
        };
        let raw = form.amount.trim().replace(',', "");
        let date = form.date.trim().to_string();
        let method = form.method();

        if raw.is_empty() {
            self.set_status("Amount is required".into());
            return;
        }
        let Ok(typed) = raw.parse::<f64>() else {
            self.set_status("Amount must be a number".into());
            return;
        };
        // The CLI's own rule, so an overpayment stays allowed and only junk is
        // refused; its message names --amount, which this form does not have.
        let amount = match payment_amount(&detail.invoice, detail.paid, Some(typed)) {
            Ok(amount) => amount,
            Err(e) => {
                self.set_status(field_wording(e.to_string()));
                return;
            }
        };
        if date.is_empty() {
            self.set_status("Date is required (YYYY-MM-DD)".into());
            return;
        }
        // A malformed date poisons refresh_status and ar_aging, so it is checked
        // through the data layer's own rule rather than one invented here.
        if let Err(e) = validate_date(&date, "payment") {
            self.set_status(e.to_string());
            return;
        }

        let invoice_id = detail.invoice.id;
        let number = detail.invoice.number;
        if let Err(e) = record_payment(conn, invoice_id, amount, &date, method, None) {
            self.set_status(e.to_string());
            return;
        }
        self.after_mutation(conn, invoice_id);
        let status = self
            .detail
            .as_ref()
            .map(|d| d.invoice.status.clone())
            .unwrap_or_default();
        self.set_status(format!(
            "Recorded {} against invoice #{number} ({status}).",
            money(amount)
        ));
    }

    /// Reload both the row list and the open detail after a write.
    fn after_mutation(&mut self, conn: &Connection, invoice_id: i64) {
        self.reload_list(conn);
        match self.load_detail(conn, invoice_id) {
            Ok(()) => self.screen = Screen::Detail,
            Err(e) => {
                self.detail = None;
                self.screen = Screen::List;
                self.set_status(e.to_string());
            }
        }
    }

    fn handle_detail_key(&mut self, code: KeyCode, conn: &Connection) -> InvoiceAction {
        match code {
            KeyCode::Up => self.detail_scroll = self.detail_scroll.saturating_sub(1),
            KeyCode::Down => self.detail_scroll += 1,
            KeyCode::PageUp => self.detail_scroll = self.detail_scroll.saturating_sub(10),
            KeyCode::PageDown => self.detail_scroll += 10,
            KeyCode::Char('p') => self.open_pay_form(&crate::cli::today()),
            KeyCode::Char('v') => self.open_void_confirmation(conn),
            KeyCode::Char('s') => {
                return self.begin_send(conn, invoicing_config(), &get_data_dir())
            }
            KeyCode::Esc => self.close_detail(),
            KeyCode::Char('q') => return InvoiceAction::Close,
            _ => {}
        }
        InvoiceAction::Continue
    }

    fn open_detail(&mut self, conn: &Connection) {
        let Some(id) = self.selected_id() else {
            return;
        };
        match self.load_detail(conn, id) {
            Ok(()) => {
                self.detail_scroll = 0;
                self.screen = Screen::Detail;
            }
            Err(e) => self.set_status(e.to_string()),
        }
    }

    fn close_detail(&mut self) {
        self.screen = Screen::List;
        self.detail = None;
    }

    /// `a` (or `n`) on the list. Refused before the form opens on a book with
    /// no clients — an invoice needs one, and the client selector would
    /// otherwise have nothing to select.
    fn open_new_form(&mut self, conn: &Connection, today: &str) {
        // Reported rather than defaulted: an unreadable clients table and an
        // empty one are opposite problems, and "No clients yet" is advice that
        // would send someone to add the client they already have.
        // Active only: `create_invoice` refuses an archived client, so the
        // selector must not offer one.
        let clients = match list_clients(conn, ClientScope::Active) {
            Ok(clients) => clients,
            Err(e) => {
                self.set_status(e.to_string());
                return;
            }
        };
        if clients.is_empty() {
            self.set_status("No clients yet. Add one on the Clients screen first.".into());
            return;
        }
        self.screen = Screen::NewInvoice(InvoiceForm::new(clients, today));
    }

    fn handle_new_key(&mut self, code: KeyCode, conn: &Connection) -> InvoiceAction {
        let Screen::NewInvoice(form) = &mut self.screen else {
            return InvoiceAction::Continue;
        };
        match code {
            KeyCode::Esc => self.screen = Screen::List,
            KeyCode::Tab | KeyCode::Down => form.focused = (form.focused + 1) % form.field_count(),
            KeyCode::BackTab | KeyCode::Up => {
                form.focused = if form.focused == 0 {
                    form.field_count() - 1
                } else {
                    form.focused - 1
                };
            }
            KeyCode::Left if form.focused == CLIENT_IDX => {
                form.client_idx = if form.client_idx == 0 {
                    form.clients.len() - 1
                } else {
                    form.client_idx - 1
                };
            }
            KeyCode::Right if form.focused == CLIENT_IDX => {
                form.client_idx = (form.client_idx + 1) % form.clients.len();
            }
            // A `Ctrl`-based binding is unavailable — the dashboard hands the
            // screen a bare `KeyCode` — and every printable character belongs
            // to the description field, so the row keys come from the
            // navigation block. `F2`/`F3` are bound alongside them because an
            // Apple keyboard has no `Insert` key at all.
            KeyCode::Insert | KeyCode::F(2) => form.add_row(),
            KeyCode::Delete | KeyCode::F(3) => form.remove_row(),
            KeyCode::Char(c) => form.push(c),
            KeyCode::Backspace => form.backspace(),
            KeyCode::Enter => self.create_from_form(conn),
            _ => {}
        }
        InvoiceAction::Continue
    }

    fn create_from_form(&mut self, conn: &Connection) {
        let Screen::NewInvoice(form) = &self.screen else {
            return;
        };
        match form.submit(conn) {
            Ok(invoice_id) => {
                self.reload_list(conn);
                self.screen = Screen::List;
                if let Some(row) = self.rows.iter().position(|r| r.id == invoice_id) {
                    self.selection = row;
                    self.ensure_visible(self.last_visible_rows);
                }
                if let Some(row) = self.rows.get(self.selection) {
                    let (number, total) = (row.number, row.total);
                    self.set_status(format!(
                        "Created draft invoice #{number} for {}.",
                        money(total)
                    ));
                }
            }
            Err((anchor, message)) => {
                if let Screen::NewInvoice(form) = &mut self.screen {
                    form.error = Some((anchor, message));
                    form.focused = anchor;
                }
            }
        }
    }

    fn handle_list_key(&mut self, code: KeyCode, conn: &Connection) -> InvoiceAction {
        // Ahead of the empty-list guard: drafting the first invoice is the one
        // thing an empty list exists to offer.
        if matches!(code, KeyCode::Char('a') | KeyCode::Char('n')) {
            self.open_new_form(conn, &crate::cli::today());
            return InvoiceAction::Continue;
        }
        if self.rows.is_empty() {
            return match code {
                KeyCode::Char('q') | KeyCode::Esc => InvoiceAction::Close,
                _ => InvoiceAction::Continue,
            };
        }
        let last = self.rows.len() - 1;
        let page = self.last_visible_rows.max(1);
        match code {
            KeyCode::Up => self.selection = self.selection.saturating_sub(1),
            KeyCode::Down => self.selection = (self.selection + 1).min(last),
            KeyCode::PageUp => self.selection = self.selection.saturating_sub(page),
            KeyCode::PageDown => self.selection = (self.selection + page).min(last),
            KeyCode::Home => self.selection = 0,
            KeyCode::End => self.selection = last,
            KeyCode::Enter => {
                self.open_detail(conn);
                return InvoiceAction::Continue;
            }
            KeyCode::Char('q') | KeyCode::Esc => return InvoiceAction::Close,
            _ => return InvoiceAction::Continue,
        }
        self.ensure_visible(self.last_visible_rows);
        InvoiceAction::Continue
    }
}

/// Throw away keys pressed while the terminal was unresponsive, so a user who
/// mashed Enter during a send does not dismiss the result before reading it.
fn drain_buffered_input() {
    while crossterm::event::poll(std::time::Duration::ZERO).unwrap_or(false) {
        let _ = crossterm::event::read();
    }
}

/// What voiding a published invoice does to what it published, wrapped to the
/// screen — the sentence `nigel invoice void` prints before the same question.
/// Empty for an invoice that was never published, which is the case with nothing
/// live behind it.
fn warning_lines(invoice: &Invoice, width: u16) -> Vec<String> {
    if invoice.published_at.is_none() {
        return Vec::new();
    }
    let (wrapped, _) = crate::tui::wrap_text(PUBLISHED_VOID_NOTICE, (width as usize).max(20) - 6);
    wrapped.lines().map(|line| format!("   {line}")).collect()
}

/// The lines S6 puts under the invoice, worded for a first send or a re-send,
/// with the `public_base_url` caution beneath them when there is one.
///
/// The warning arrives as an argument rather than being read here: this is the
/// pure half, and the sentence itself lives in `invoicing::r2` so a terminal and
/// a browser say the same thing about the same setting.
fn send_confirmation(detail: &Detail, base_url_warning: Option<&str>, width: u16) -> Vec<String> {
    let invoice = &detail.invoice;
    let email = optional_display(detail.client_email());
    let mut lines = match &invoice.published_at {
        Some(published) => vec![
            format!("Re-send invoice #{} to {email}?", invoice.number),
            format!("Published {published}. The existing payment link is reused; the page and"),
            "PDF are republished and the client is emailed again.".to_string(),
        ],
        None => vec![
            format!("Send invoice #{} to {email}?", invoice.number),
            format!(
                "{} \u{b7} {}. Creates a Stripe payment link, publishes the",
                detail.client_name(),
                money(invoice.total)
            ),
            "page and PDF, then emails the client.".to_string(),
        ],
    };
    if let Some(warning) = base_url_warning {
        let (wrapped, _) = crate::tui::wrap_text(warning, (width as usize).max(20) - 6);
        lines.extend(wrapped.lines().map(|line| format!("notice: {line}")));
    }
    lines
}

/// The CLI names the flag it wants; a form names the field that was typed in.
fn field_wording(message: String) -> String {
    match message.strip_prefix("--amount ") {
        Some(rest) => format!("Amount {rest}"),
        None => message,
    }
}

/// One right-aligned label/amount row under the line items.
fn total_line(label: &str, amount: f64) -> Line<'static> {
    Line::from(format!("   {:>44} {:>15}", label, money(amount)))
}

const DRAFT_HINT: &str = "A draft is not sent. Open it and press s to send it.";

/// Where a header field's value starts, and where a line item's description
/// does — a refusal is indented to its field so it reads as belonging to it.
const FIELD_VALUE_COLUMN: usize = 18;
const ITEM_TEXT_COLUMN: usize = 7;

/// The refusal for one field, or nothing. Rendered as its own line directly
/// beneath the field, which is what makes it read as beside that field rather
/// than as a status line about the form as a whole.
fn error_line(form: &InvoiceForm, field: usize, indent: usize) -> Option<Line<'static>> {
    let (anchor, message) = form.error.as_ref()?;
    if *anchor != field {
        return None;
    }
    Some(Line::from(Span::styled(
        format!("{:indent$}{message}", ""),
        Style::default().fg(Color::Yellow),
    )))
}

/// The line-item table's column budget, fitted to the **whole** table — the
/// header, every row and the total — so those three cannot drift apart, and
/// held to `ROW_WIDTH`.
///
/// The **description yields**, which is `list_cells`' call for the invoice
/// list: an eight-figure unit amount needs a wider figure column, and taking
/// that width from the description is what keeps the figure itself on screen.
/// Truncating a number would be the one thing worse than a wrapped row.
#[derive(Clone, Copy)]
struct ItemColumns {
    number: usize,
    description: usize,
    quantity: usize,
    unit: usize,
    amount: usize,
}

/// The narrowest a description is allowed to get before the row is simply
/// allowed to overrun, matching `list_cells`' own floor.
const MIN_DESCRIPTION: usize = 8;

impl ItemColumns {
    fn fit(rows: &[[String; 5]]) -> Self {
        let width = |column: usize, min: usize| {
            rows.iter()
                .map(|row| row[column].chars().count())
                .max()
                .unwrap_or(0)
                .max(min)
        };
        let (number, quantity, unit, amount) =
            (width(0, 2), width(2, 8), width(3, 11), width(4, 12));
        // The three-space marker, the two spaces after the row number, and one
        // space before each of the three figure columns.
        let fixed = 3 + number + 2 + 3 + quantity + unit + amount;
        Self {
            number,
            description: ROW_WIDTH.saturating_sub(fixed).max(MIN_DESCRIPTION),
            quantity,
            unit,
            amount,
        }
    }

    /// One row's five cells, padded to the budget, in column order.
    fn pad(&self, row: &[String; 5]) -> [String; 5] {
        [
            format!("   {:>width$}  ", row[0], width = self.number),
            format!(
                "{:<width$}",
                truncate(&row[1], self.description),
                width = self.description
            ),
            format!(" {:>width$}", row[2], width = self.quantity),
            format!(" {:>width$}", row[3], width = self.unit),
            format!(" {:>width$}", row[4], width = self.amount),
        ]
    }
}

fn header_cells() -> [String; 5] {
    ["#", "Description", "Qty", "Unit", "Amount"].map(str::to_string)
}

fn total_cells(total: f64) -> [String; 5] {
    [
        String::new(),
        String::new(),
        String::new(),
        "Total".to_string(),
        money(total),
    ]
}

/// One row's cell text, cursor included, before any padding. The amount is
/// derived, never typed.
fn item_cells(form: &InvoiceForm, row: usize) -> [String; 5] {
    let item = &form.items[row];
    let focused = focused_cell_of(form, row);
    let typed = |cell: usize, value: &str| {
        if focused == Some(cell) {
            format!("{value}_")
        } else {
            value.to_string()
        }
    };
    [
        (row + 1).to_string(),
        typed(DESC_CELL, &item.description),
        typed(QTY_CELL, &item.quantity),
        typed(UNIT_CELL, &item.unit_amount),
        item.amount()
            .map_or_else(|| "\u{2014}".to_string(), |a| format!("{a:.2}")),
    ]
}

fn focused_cell_of(form: &InvoiceForm, row: usize) -> Option<usize> {
    form.focused_cell()
        .and_then(|(r, cell)| (r == row).then_some(cell))
}

/// Every cell of the line-item table, plus the widths fitted to all of it.
fn item_table(form: &InvoiceForm) -> (Vec<[String; 5]>, ItemColumns) {
    let mut rows = vec![header_cells()];
    rows.extend((0..form.items.len()).map(|row| item_cells(form, row)));
    rows.push(total_cells(form.total()));
    let columns = ItemColumns::fit(&rows);
    (rows, columns)
}

/// One editable row: the focused cell is cyan, everything derived is dim.
fn item_line(form: &InvoiceForm, row: usize, cells: &[String; 5]) -> Line<'static> {
    let focused = focused_cell_of(form, row);
    let style = |cell: usize| {
        if focused == Some(cell) {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default()
        }
    };
    let dim = Style::default().fg(Color::DarkGray);
    Line::from(vec![
        Span::styled(cells[0].clone(), dim),
        Span::styled(cells[1].clone(), style(DESC_CELL)),
        Span::styled(cells[2].clone(), style(QTY_CELL)),
        Span::styled(cells[3].clone(), style(UNIT_CELL)),
        Span::styled(cells[4].clone(), dim),
    ])
}

/// The whole table as plain strings. A rendered frame is `ROW_WIDTH` cells wide
/// by construction, so a row that overruns is invisible in a `TestBackend`
/// buffer — the budget is checked here, on the string, exactly as `list_row`
/// checks the invoice list's.
#[cfg(test)]
fn item_table_strings(form: &InvoiceForm) -> Vec<String> {
    let (rows, columns) = item_table(form);
    rows.iter().map(|row| columns.pad(row).concat()).collect()
}

/// An absent value reads as an em dash, never as an invented blank.
fn optional_display(value: Option<&str>) -> String {
    match value.map(str::trim) {
        Some(v) if !v.is_empty() => v.to_string(),
        _ => "\u{2014}".to_string(),
    }
}

/// What is still owed on an invoice.
fn balance(row: &InvoiceListRow) -> f64 {
    row.balance
}

/// The column budget S4 lays out. A `TestBackend` buffer is this wide by
/// construction, so a row that overruns is invisible in a rendered frame —
/// `list_row` is where the budget is actually checked.
const ROW_WIDTH: usize = 80;

/// A list row as three cells: everything before the status, the status itself
/// (the only coloured one), and everything after it.
///
/// The **client name is the cell that yields**: a seven-figure total needs a
/// wider money column, and taking that width from the client rather than
/// letting the row grow is what keeps the due date on screen.
fn list_cells(marker: &str, row: &InvoiceListRow) -> (String, String, String) {
    let total = money(row.total);
    let paid = money(balance(row));
    let due = row.due_date.as_deref().unwrap_or("\u{2014}");

    let money_width = total.chars().count().max(paid.chars().count()).max(12);
    let due_width = due.chars().count().max(10);
    // marker 3, number 6, status, the two money cells, the due date, and the
    // five single-space gaps between them.
    let fixed = 3 + 6 + STATUS_WIDTH + 5 + 2 * money_width + due_width;
    let client_width = ROW_WIDTH.saturating_sub(fixed).max(8);

    (
        format!("{marker}{:<6} ", row.number),
        format!("{:<STATUS_WIDTH$} ", truncate(&row.status, STATUS_WIDTH)),
        format!(
            "{:<client_width$} {total:>money_width$} {paid:>money_width$} {due}",
            truncate(
                &optional_display(row.client_name.as_deref()),
                client_width.saturating_sub(2)
            ),
        ),
    )
}

const STATUS_WIDTH: usize = 8;

#[cfg(test)]
fn list_row(row: &InvoiceListRow) -> String {
    let (number, status, rest) = list_cells("   ", row);
    format!("{number}{status}{rest}")
}

/// Colour carries status, since every figure on this screen is a positive
/// receivable. The column is TEXT, so an unrecognized value renders plain
/// rather than panicking.
fn status_style(status: &str) -> Style {
    match status {
        "draft" | "void" => Style::default().fg(Color::DarkGray),
        "sent" => Style::default().fg(Color::Cyan),
        "partial" => Style::default().fg(Color::Yellow),
        "paid" => Style::default().fg(GREEN),
        "overdue" => Style::default().fg(Color::Red),
        _ => Style::default(),
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
    use crate::db::{get_connection, init_db};
    use crate::invoicing::clients::add_client;
    use crate::invoicing::invoices::{
        create_invoice, mark_published, record_payment, set_payment_link, void_invoice, NewLineItem,
    };
    use crate::migrations::run_migrations;

    fn test_conn() -> (tempfile::TempDir, Connection) {
        let dir = tempfile::tempdir().unwrap();
        let conn = get_connection(&dir.path().join("t.db")).unwrap();
        init_db(&conn).unwrap();
        run_migrations(&conn).unwrap();
        (dir, conn)
    }

    fn manager(conn: &Connection) -> InvoiceManager {
        InvoiceManager::new(conn, "Hello, Sam.")
    }

    fn is_close(action: InvoiceAction) -> bool {
        matches!(action, InvoiceAction::Close)
    }

    /// A client and one invoice of `amount`, returning the invoice row id.
    fn seed_invoice(conn: &Connection, client: &str, amount: f64) -> i64 {
        let cid = add_client(conn, client, Some("ops@cedar.test"), None, None).unwrap();
        seed_invoice_for(conn, cid, amount)
    }

    fn seed_invoice_for(conn: &Connection, client_id: i64, amount: f64) -> i64 {
        let items = vec![NewLineItem {
            description: "Strategy workshop".into(),
            quantity: 1.0,
            unit_amount: amount,
        }];
        create_invoice(
            conn,
            client_id,
            "2026-07-16",
            Some("2026-08-15"),
            "USD",
            &items,
            None,
            None,
        )
        .unwrap()
    }

    fn seed_three(conn: &Connection) -> Vec<i64> {
        let cid = add_client(conn, "Cedar Systems", Some("ops@cedar.test"), None, None).unwrap();
        [100.0, 200.0, 300.0]
            .into_iter()
            .map(|amount| seed_invoice_for(conn, cid, amount))
            .collect()
    }

    #[test]
    fn new_loads_invoices_newest_first() {
        let (_d, conn) = test_conn();
        seed_three(&conn);
        let mgr = manager(&conn);

        let numbers: Vec<i64> = mgr.rows.iter().map(|r| r.number).collect();
        assert_eq!(numbers, [1250, 1249, 1248]);
    }

    #[test]
    fn new_on_an_empty_book_does_not_panic() {
        let (_d, conn) = test_conn();
        let mgr = manager(&conn);
        assert!(mgr.rows.is_empty());
        assert_eq!(mgr.selection, 0);
    }

    #[test]
    fn navigation_clamps_at_both_ends() {
        let (_d, conn) = test_conn();
        seed_three(&conn);
        let mut mgr = manager(&conn);

        mgr.handle_key(KeyCode::End, &conn);
        assert_eq!(mgr.selection, 2);
        mgr.handle_key(KeyCode::Down, &conn);
        assert_eq!(mgr.selection, 2);
        mgr.handle_key(KeyCode::PageDown, &conn);
        assert_eq!(mgr.selection, 2);
        mgr.handle_key(KeyCode::Home, &conn);
        assert_eq!(mgr.selection, 0);
        mgr.handle_key(KeyCode::Up, &conn);
        assert_eq!(mgr.selection, 0);
        mgr.handle_key(KeyCode::PageUp, &conn);
        assert_eq!(mgr.selection, 0);
        mgr.handle_key(KeyCode::PageDown, &conn);
        assert_eq!(
            mgr.selection, 2,
            "a page longer than the list lands on the end"
        );
    }

    #[test]
    fn esc_and_q_close_from_the_list() {
        let (_d, conn) = test_conn();
        seed_three(&conn);
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
            KeyCode::End,
            KeyCode::PageDown,
            KeyCode::Enter,
        ] {
            assert!(!is_close(mgr.handle_key(code, &conn)));
        }
        assert!(is_close(mgr.handle_key(KeyCode::Esc, &conn)));
    }

    #[test]
    fn status_style_maps_every_invoice_status() {
        let expected = [
            ("draft", Color::DarkGray),
            ("sent", Color::Cyan),
            ("partial", Color::Yellow),
            ("paid", GREEN),
            ("overdue", Color::Red),
            ("void", Color::DarkGray),
        ];
        for (status, colour) in expected {
            assert_eq!(status_style(status).fg, Some(colour), "status {status}");
        }
        // The column is TEXT, not an enum.
        assert_eq!(status_style("something-else"), Style::default());
    }

    #[test]
    fn balance_is_total_minus_paid() {
        let (_d, conn) = test_conn();
        let id = seed_invoice(&conn, "Cedar Systems", 2_000.0);
        record_payment(&conn, id, 1_250.0, "2026-08-01", "ach", None).unwrap();

        let mgr = manager(&conn);
        let row = &mgr.rows[0];
        assert_eq!(balance(row), 750.0);
    }

    fn detail_of(mgr: &InvoiceManager) -> &Detail {
        mgr.detail.as_deref().expect("no detail loaded")
    }

    #[test]
    fn enter_loads_the_detail_for_the_selected_invoice() {
        let (_d, conn) = test_conn();
        let id = seed_invoice(&conn, "Cedar Systems", 2_000.0);
        record_payment(&conn, id, 1_250.0, "2026-08-01", "ach", None).unwrap();
        let mut mgr = manager(&conn);
        mgr.handle_key(KeyCode::Enter, &conn);

        assert!(matches!(mgr.screen, Screen::Detail));
        let detail = detail_of(&mgr);
        assert_eq!(detail.invoice.number, 1248);
        assert_eq!(detail.client_name(), "Cedar Systems");
        assert_eq!(detail.items.len(), 1);
        assert_eq!(detail.payments.len(), 1);
        assert_eq!(detail.paid, 1_250.0);
        assert_eq!(detail.balance(), 750.0);
    }

    /// The state is unreachable through Nigel — `delete_client` refuses a
    /// client that has invoices of any status, and the foreign key refuses it
    /// again — but it is representable, and refusing to open the detail would
    /// hide the very invoice that needs looking at.
    #[test]
    fn an_invoice_whose_client_row_is_gone_still_opens() {
        let (_d, conn) = test_conn();
        seed_invoice(&conn, "Cedar Systems", 2_000.0);
        conn.execute("PRAGMA foreign_keys = OFF", []).unwrap();
        conn.execute("DELETE FROM clients", []).unwrap();

        let mut mgr = manager(&conn);
        mgr.handle_key(KeyCode::Enter, &conn);

        assert!(matches!(mgr.screen, Screen::Detail));
        let detail = detail_of(&mgr);
        assert!(detail.client.is_none());
        assert_eq!(detail.client_name(), "\u{2014}");
        assert_eq!(detail.client_email(), None);

        let screen = rendered(&mut mgr);
        assert!(screen.contains("Client    \u{2014}"), "{screen}");
    }

    #[test]
    fn esc_from_detail_returns_to_the_list_without_closing_the_screen() {
        let (_d, conn) = test_conn();
        seed_invoice(&conn, "Cedar Systems", 100.0);
        let mut mgr = manager(&conn);
        mgr.handle_key(KeyCode::Enter, &conn);

        assert!(!is_close(mgr.handle_key(KeyCode::Esc, &conn)));
        assert!(matches!(mgr.screen, Screen::List));
        assert!(mgr.detail.is_none());
    }

    #[test]
    fn q_from_detail_leaves_the_screen() {
        let (_d, conn) = test_conn();
        seed_invoice(&conn, "Cedar Systems", 100.0);
        let mut mgr = manager(&conn);
        mgr.handle_key(KeyCode::Enter, &conn);
        assert!(is_close(mgr.handle_key(KeyCode::Char('q'), &conn)));
    }

    #[test]
    fn enter_on_an_empty_list_does_nothing() {
        let (_d, conn) = test_conn();
        let mut mgr = manager(&conn);
        mgr.handle_key(KeyCode::Enter, &conn);
        assert!(matches!(mgr.screen, Screen::List));
        assert!(mgr.detail.is_none());
    }

    #[test]
    fn detail_scroll_clamps_at_zero() {
        let (_d, conn) = test_conn();
        seed_invoice(&conn, "Cedar Systems", 100.0);
        let mut mgr = manager(&conn);
        mgr.handle_key(KeyCode::Enter, &conn);

        mgr.handle_key(KeyCode::Up, &conn);
        assert_eq!(mgr.detail_scroll, 0);
        mgr.handle_key(KeyCode::PageUp, &conn);
        assert_eq!(mgr.detail_scroll, 0);
        mgr.handle_key(KeyCode::Down, &conn);
        assert_eq!(mgr.detail_scroll, 1);
    }

    #[test]
    fn detail_scroll_stops_at_the_last_screenful() {
        let (_d, conn) = test_conn();
        seed_invoice(&conn, "Cedar Systems", 100.0);
        let mut mgr = manager(&conn);
        mgr.handle_key(KeyCode::Enter, &conn);

        for _ in 0..20 {
            mgr.handle_key(KeyCode::PageDown, &conn);
        }
        let screen = rendered(&mut mgr);
        assert!(
            screen.contains("Invoice #1248"),
            "a short invoice scrolled off its own screen:\n{screen}"
        );
        // One Up from the clamped position must move, not undo 200 rows.
        mgr.handle_key(KeyCode::Up, &conn);
        assert_eq!(mgr.detail_scroll, 0);
    }

    #[test]
    fn the_detail_renders_the_invoice_its_payments_and_the_action_keys() {
        let (_d, conn) = test_conn();
        let id = seed_invoice(&conn, "Cedar Systems", 2_000.0);
        record_payment(&conn, id, 1_250.0, "2026-08-01", "ach", None).unwrap();
        set_payment_link(&conn, id, "pl_1", "https://pay/x").unwrap();
        let mut mgr = manager(&conn);
        mgr.handle_key(KeyCode::Enter, &conn);

        let screen = rendered(&mut mgr);
        assert!(screen.contains("Invoice #1248"), "{screen}");
        assert!(screen.contains("Cedar Systems"), "{screen}");
        assert!(screen.contains("Strategy workshop"), "{screen}");
        assert!(screen.contains("Payments"), "{screen}");
        assert!(screen.contains("2026-08-01"), "{screen}");
        assert!(screen.contains("Balance"), "{screen}");
        assert!(screen.contains("$750.00"), "{screen}");
        assert!(screen.contains("Pay link  https://pay/x"), "{screen}");
        assert!(
            screen.contains("s=send  p=record payment  v=void"),
            "{screen}"
        );
    }

    #[test]
    fn an_unpaid_invoice_has_no_payments_section_and_no_pay_link() {
        let (_d, conn) = test_conn();
        seed_invoice(&conn, "Cedar Systems", 1_250.0);
        let mut mgr = manager(&conn);
        mgr.handle_key(KeyCode::Enter, &conn);

        let screen = rendered(&mut mgr);
        assert!(!screen.contains("Payments"), "{screen}");
        assert!(!screen.contains("Pay link"), "{screen}");
        assert!(screen.contains("Paid"), "{screen}");
    }

    #[test]
    fn a_void_invoice_shows_its_void_date_and_drops_the_action_keys() {
        let (_d, conn) = test_conn();
        let id = seed_invoice(&conn, "Cedar Systems", 100.0);
        void_invoice(&conn, id, "2026-08-07").unwrap();
        let mut mgr = manager(&conn);
        mgr.handle_key(KeyCode::Enter, &conn);

        let screen = rendered(&mut mgr);
        assert!(screen.contains("Voided    2026-08-07"), "{screen}");
        assert!(!screen.contains("s=send"), "{screen}");
        assert!(
            screen.contains("Up/Down=scroll  Esc=back  q=quit"),
            "{screen}"
        );
    }

    fn pay_form(mgr: &InvoiceManager) -> &PayForm {
        match &mgr.screen {
            Screen::PayForm(form) => form,
            _ => panic!("not on the payment form"),
        }
    }

    /// Open the detail for the only invoice and press `p`.
    fn open_pay(mgr: &mut InvoiceManager, conn: &Connection) {
        mgr.handle_key(KeyCode::Enter, conn);
        mgr.handle_key(KeyCode::Char('p'), conn);
    }

    fn type_str(mgr: &mut InvoiceManager, conn: &Connection, text: &str) {
        for ch in text.chars() {
            mgr.handle_key(KeyCode::Char(ch), conn);
        }
    }

    fn clear_field(mgr: &mut InvoiceManager, conn: &Connection) {
        for _ in 0..40 {
            mgr.handle_key(KeyCode::Backspace, conn);
        }
    }

    fn payment_rows(conn: &Connection) -> i64 {
        conn.query_row("SELECT COUNT(*) FROM invoice_payments", [], |r| r.get(0))
            .unwrap()
    }

    #[test]
    fn p_on_a_void_invoice_is_refused_before_the_form_opens() {
        let (_d, conn) = test_conn();
        let id = seed_invoice(&conn, "Cedar Systems", 100.0);
        void_invoice(&conn, id, "2026-08-07").unwrap();
        let mut mgr = manager(&conn);
        open_pay(&mut mgr, &conn);

        assert!(matches!(mgr.screen, Screen::Detail));
        assert_eq!(
            mgr.status_message.as_deref(),
            Some("Invoice #1248 is void and cannot be paid.")
        );
    }

    #[test]
    fn p_prefills_the_amount_with_the_outstanding_balance_and_today() {
        let (_d, conn) = test_conn();
        let id = seed_invoice(&conn, "Cedar Systems", 2_000.0);
        record_payment(&conn, id, 1_250.0, "2026-08-01", "ach", None).unwrap();
        let mut mgr = manager(&conn);
        open_pay(&mut mgr, &conn);

        let form = pay_form(&mgr);
        assert_eq!(form.amount, "750.00");
        assert_eq!(form.date, crate::cli::today());
    }

    #[test]
    fn p_on_a_settled_invoice_prefills_an_empty_amount() {
        let (_d, conn) = test_conn();
        let id = seed_invoice(&conn, "Cedar Systems", 100.0);
        record_payment(&conn, id, 100.0, "2026-08-01", "ach", None).unwrap();
        let mut mgr = manager(&conn);
        open_pay(&mut mgr, &conn);

        assert_eq!(pay_form(&mgr).amount, "");
    }

    #[test]
    fn method_options_are_exactly_the_four_the_schema_allows() {
        assert_eq!(METHODS, ["direct_deposit", "ach", "stripe", "other"]);
        let (_d, conn) = test_conn();
        seed_invoice(&conn, "Cedar Systems", 100.0);
        let mut mgr = manager(&conn);
        open_pay(&mut mgr, &conn);
        assert_eq!(pay_form(&mgr).method(), "direct_deposit");
    }

    #[test]
    fn every_method_option_is_actually_insertable() {
        // invoice_payments.method carries a CHECK constraint; a fifth option
        // would fail at insert time, not at compile time.
        let (_d, conn) = test_conn();
        let id = seed_invoice(&conn, "Cedar Systems", 1_000.0);
        for method in METHODS {
            record_payment(&conn, id, 1.0, "2026-08-01", method, None)
                .unwrap_or_else(|e| panic!("method {method} is not insertable: {e}"));
        }
    }

    #[test]
    fn left_and_right_cycle_the_method() {
        let (_d, conn) = test_conn();
        seed_invoice(&conn, "Cedar Systems", 100.0);
        let mut mgr = manager(&conn);
        open_pay(&mut mgr, &conn);
        for _ in 0..METHOD_IDX {
            mgr.handle_key(KeyCode::Tab, &conn);
        }
        mgr.handle_key(KeyCode::Tab, &conn);
        mgr.handle_key(KeyCode::Tab, &conn);
        while pay_form(&mgr).focused != METHOD_IDX {
            mgr.handle_key(KeyCode::Tab, &conn);
        }

        mgr.handle_key(KeyCode::Right, &conn);
        assert_eq!(pay_form(&mgr).method(), "ach");
        mgr.handle_key(KeyCode::Left, &conn);
        assert_eq!(pay_form(&mgr).method(), "direct_deposit");
        mgr.handle_key(KeyCode::Left, &conn);
        assert_eq!(pay_form(&mgr).method(), "other", "the selector wraps");
    }

    /// Type an amount and a date into a freshly opened form, then Enter.
    fn submit_payment(mgr: &mut InvoiceManager, conn: &Connection, amount: &str, date: &str) {
        clear_field(mgr, conn);
        type_str(mgr, conn, amount);
        mgr.handle_key(KeyCode::Tab, conn);
        clear_field(mgr, conn);
        type_str(mgr, conn, date);
        mgr.handle_key(KeyCode::Enter, conn);
    }

    #[test]
    fn the_validation_table_refuses_and_writes_nothing() {
        // Only what the fields actually accept: they take digits, `.` and `,`
        // for the amount and digits and `-` for the date, so a letter never
        // reaches validation.
        let cases: [(&str, &str, &str); 6] = [
            ("", "2026-08-07", "Amount is required"),
            (".", "2026-08-07", "Amount must be a number"),
            ("0", "2026-08-07", "Amount must be a finite number"),
            ("0.00", "2026-08-07", "Amount must be a finite number"),
            ("100", "", "Date is required (YYYY-MM-DD)"),
            (
                "100",
                "2026-13-45",
                "Invalid payment date: 2026-13-45 (expected YYYY-MM-DD)",
            ),
        ];
        for (amount, date, expected) in cases {
            let (_d, conn) = test_conn();
            seed_invoice(&conn, "Cedar Systems", 2_000.0);
            let mut mgr = manager(&conn);
            open_pay(&mut mgr, &conn);
            submit_payment(&mut mgr, &conn, amount, date);

            let message = mgr.status_message.clone().unwrap_or_default();
            assert!(
                message.contains(expected),
                "amount {amount:?} date {date:?}: expected {expected:?}, got {message:?}"
            );
            assert!(matches!(mgr.screen, Screen::PayForm(_)), "form stayed open");
            assert_eq!(payment_rows(&conn), 0, "amount {amount:?} date {date:?}");
        }
    }

    #[test]
    fn a_malformed_date_is_refused_in_the_data_layers_wording() {
        let (_d, conn) = test_conn();
        seed_invoice(&conn, "Cedar Systems", 100.0);
        let mut mgr = manager(&conn);
        open_pay(&mut mgr, &conn);
        submit_payment(&mut mgr, &conn, "100", "2026-13-45");

        assert_eq!(
            mgr.status_message.as_deref(),
            Some("Invalid payment date: 2026-13-45 (expected YYYY-MM-DD)")
        );
        assert_eq!(payment_rows(&conn), 0);
    }

    #[test]
    fn a_negative_or_non_finite_amount_is_refused_in_the_cli_s_words() {
        // Unreachable by typing — the field takes no `-` and no letters — but
        // the rule is the CLI's, so the screen cannot disagree about it.
        let (_d, conn) = test_conn();
        seed_invoice(&conn, "Cedar Systems", 100.0);
        let invoice = get_invoice(&conn, list_invoices(&conn, None, None).unwrap()[0].id).unwrap();

        for amount in [-25.0, f64::NAN, f64::INFINITY] {
            let message = field_wording(
                payment_amount(&invoice, 0.0, Some(amount))
                    .unwrap_err()
                    .to_string(),
            );
            assert!(
                message.starts_with("Amount must be a finite number greater than zero"),
                "got: {message}"
            );
        }
    }

    #[test]
    fn the_amount_refusal_names_the_field_not_the_cli_flag() {
        let (_d, conn) = test_conn();
        seed_invoice(&conn, "Cedar Systems", 100.0);
        let mut mgr = manager(&conn);
        open_pay(&mut mgr, &conn);
        submit_payment(&mut mgr, &conn, "0", "2026-08-07");

        let message = mgr.status_message.clone().unwrap();
        assert!(!message.contains("--amount"), "got: {message}");
        assert!(message.starts_with("Amount must be"), "got: {message}");
    }

    #[test]
    fn a_valid_payment_is_recorded_and_returns_to_detail() {
        let (_d, conn) = test_conn();
        let id = seed_invoice(&conn, "Cedar Systems", 2_000.0);
        record_payment(&conn, id, 1_250.0, "2026-08-01", "ach", None).unwrap();
        let mut mgr = manager(&conn);
        open_pay(&mut mgr, &conn);
        submit_payment(&mut mgr, &conn, "750.00", "2026-08-20");

        assert!(matches!(mgr.screen, Screen::Detail));
        let (amount, date, method): (f64, String, String) = conn
            .query_row(
                "SELECT amount, paid_date, method FROM invoice_payments ORDER BY id DESC LIMIT 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        assert_eq!(
            (amount, date.as_str(), method.as_str()),
            (750.0, "2026-08-20", "direct_deposit")
        );

        let detail = detail_of(&mgr);
        assert_eq!(detail.paid, 2_000.0);
        assert_eq!(detail.balance(), 0.0);
        assert_eq!(detail.invoice.status, "paid");
        assert_eq!(mgr.rows[0].paid, 2_000.0, "the list reloaded too");
        assert_eq!(
            mgr.status_message.as_deref(),
            Some("Recorded $750.00 against invoice #1248 (paid).")
        );
    }

    #[test]
    fn an_overpayment_is_allowed() {
        let (_d, conn) = test_conn();
        seed_invoice(&conn, "Cedar Systems", 100.0);
        let mut mgr = manager(&conn);
        open_pay(&mut mgr, &conn);
        submit_payment(&mut mgr, &conn, "250", "2026-08-07");

        assert_eq!(payment_rows(&conn), 1);
        assert_eq!(detail_of(&mgr).invoice.status, "paid");
    }

    #[test]
    fn commas_are_stripped_from_the_amount() {
        let (_d, conn) = test_conn();
        seed_invoice(&conn, "Cedar Systems", 2_000.0);
        let mut mgr = manager(&conn);
        open_pay(&mut mgr, &conn);
        submit_payment(&mut mgr, &conn, "1,250.00", "2026-08-07");

        assert_eq!(detail_of(&mgr).paid, 1_250.0);
    }

    #[test]
    fn the_amount_field_refuses_letters_and_the_date_field_refuses_slashes() {
        let (_d, conn) = test_conn();
        seed_invoice(&conn, "Cedar Systems", 100.0);
        let mut mgr = manager(&conn);
        open_pay(&mut mgr, &conn);
        clear_field(&mut mgr, &conn);
        type_str(&mut mgr, &conn, "1a2");
        mgr.handle_key(KeyCode::Tab, &conn);
        clear_field(&mut mgr, &conn);
        type_str(&mut mgr, &conn, "2026/08/07");

        assert_eq!(pay_form(&mgr).amount, "12");
        assert_eq!(pay_form(&mgr).date, "20260807");
    }

    #[test]
    fn esc_cancels_the_payment_without_writing() {
        let (_d, conn) = test_conn();
        seed_invoice(&conn, "Cedar Systems", 100.0);
        let mut mgr = manager(&conn);
        open_pay(&mut mgr, &conn);
        mgr.handle_key(KeyCode::Esc, &conn);

        assert!(matches!(mgr.screen, Screen::Detail));
        assert_eq!(payment_rows(&conn), 0);
    }

    #[test]
    fn the_pay_form_renders_the_balance_line_and_the_fields() {
        let (_d, conn) = test_conn();
        let id = seed_invoice(&conn, "Cedar Systems", 2_000.0);
        record_payment(&conn, id, 1_250.0, "2026-08-01", "ach", None).unwrap();
        let mut mgr = manager(&conn);
        open_pay(&mut mgr, &conn);

        let screen = rendered(&mut mgr);
        assert!(screen.contains("Record a Payment"), "{screen}");
        assert!(screen.contains("#1248"), "{screen}");
        assert!(screen.contains("$1,250.00"), "{screen}");
        assert!(screen.contains("$750.00"), "{screen}");
        assert!(screen.contains("direct_deposit"), "{screen}");
        assert!(
            screen.contains("Tab=next field  Left/Right=method  Enter=record  Esc=cancel"),
            "{screen}"
        );
    }

    /// The `/i` caution is a fact about the installation, so it belongs where
    /// the operator is deciding — under the send confirmation, not only on the
    /// CLI's stderr.
    #[test]
    fn the_send_confirmation_carries_the_public_base_url_notice() {
        let (_d, conn) = test_conn();
        seed_invoice(&conn, "Cedar Systems", 100.0);
        let mut mgr = manager(&conn);
        mgr.handle_key(KeyCode::Enter, &conn);
        let detail = mgr.detail.as_ref().expect("a detail is open");

        let quiet = send_confirmation(detail, None, 80);
        assert!(
            !quiet.iter().any(|line| line.starts_with("notice:")),
            "a configured installation says nothing extra: {quiet:?}"
        );

        let warning = crate::invoicing::r2::public_base_url_warning("https://billing.example.test")
            .expect("a base URL missing the prefix warns");
        let warned = send_confirmation(detail, Some(warning), 80);
        assert_eq!(
            &warned[..quiet.len()],
            &quiet[..],
            "the notice is added below the decision, never in place of it"
        );
        let notice = warned[quiet.len()..].join(" ");
        assert!(notice.starts_with("notice:"), "{warned:?}");
        assert!(notice.contains("/i"), "{warned:?}");
    }

    fn open_void(mgr: &mut InvoiceManager, conn: &Connection) {
        mgr.handle_key(KeyCode::Enter, conn);
        mgr.handle_key(KeyCode::Char('v'), conn);
    }

    fn voided_at(conn: &Connection) -> Option<String> {
        conn.query_row("SELECT voided_at FROM invoices LIMIT 1", [], |r| r.get(0))
            .unwrap()
    }

    #[test]
    fn v_opens_the_confirmation_naming_the_invoice_client_and_total() {
        let (_d, conn) = test_conn();
        seed_invoice(&conn, "Acme Co", 1_250.0);
        let mut mgr = manager(&conn);
        open_void(&mut mgr, &conn);

        assert!(matches!(mgr.screen, Screen::ConfirmVoid));
        let screen = rendered(&mut mgr);
        assert!(
            screen.contains("Void invoice #1248 for Acme Co ($1,250.00)?"),
            "{screen}"
        );
        assert!(screen.contains("Void is permanent."), "{screen}");
        assert!(screen.contains("y=void  n=cancel"), "{screen}");
    }

    #[test]
    fn n_and_esc_cancel_without_writing() {
        for key in [KeyCode::Char('n'), KeyCode::Esc] {
            let (_d, conn) = test_conn();
            seed_invoice(&conn, "Acme Co", 100.0);
            let mut mgr = manager(&conn);
            open_void(&mut mgr, &conn);
            mgr.handle_key(key, &conn);

            assert!(matches!(mgr.screen, Screen::Detail), "{key:?}");
            assert_eq!(voided_at(&conn), None, "{key:?}");
        }
    }

    /// Answer the void confirmation, then run the work the controller runs
    /// after painting the frame — through `perform_pending_with`, so the
    /// dispatch that tells a pending void from a pending send is exercised too.
    /// Nothing is configured here, which is the case these tests are about.
    fn confirm_void(mgr: &mut InvoiceManager, conn: &Connection) {
        match mgr.handle_key(KeyCode::Char('y'), conn) {
            // Nothing live behind the invoice: the void ran inline, with no
            // blocking frame to paint.
            InvoiceAction::Continue => assert!(
                !matches!(mgr.screen, Screen::Voiding),
                "an inline void enters no blocking state"
            ),
            InvoiceAction::Perform => {
                assert!(
                    matches!(mgr.screen, Screen::Voiding),
                    "the frame is painted"
                );
                // Through `perform_pending_with`, so the dispatch that tells a
                // pending void from a pending send is exercised too.
                mgr.perform_pending_with(
                    conn,
                    &crate::cli::today(),
                    no_config(),
                    std::path::Path::new("/nonexistent"),
                );
            }
            InvoiceAction::Close => panic!("void never closes the screen"),
        }
    }

    #[test]
    fn y_voids_and_reloads_the_detail() {
        let (_d, conn) = test_conn();
        seed_invoice(&conn, "Acme Co", 100.0);
        let mut mgr = manager(&conn);
        open_void(&mut mgr, &conn);
        confirm_void(&mut mgr, &conn);

        assert!(matches!(mgr.screen, Screen::Detail));
        assert_eq!(detail_of(&mgr).invoice.status, "void");
        assert_eq!(mgr.rows[0].status, "void", "the list reloaded too");
        assert_eq!(mgr.status_message.as_deref(), Some("Voided invoice #1248."));

        let screen = rendered(&mut mgr);
        assert!(
            !screen.contains("s=send"),
            "the actions are gone:\n{screen}"
        );
    }

    #[test]
    fn voiding_a_published_invoice_says_what_void_will_do_to_what_it_published() {
        let (_d, conn) = test_conn();
        let id = seed_invoice(&conn, "Cedar Systems", 100.0);
        mark_published(&conn, id, "2026-07-16").unwrap();
        let mut mgr = manager(&conn);
        open_void(&mut mgr, &conn);

        // Before the decision: the confirmation carries the CLI's own notice.
        let dialog = rendered(&mut mgr);
        // Fragments short enough to survive the wrap, whatever the width.
        assert!(dialog.contains("already published"), "{dialog}");
        assert!(dialog.contains("replaces the published page"), "{dialog}");

        // Nothing is configured, so the page it names is still up — and the
        // result screen says so rather than passing the void off as complete.
        confirm_void(&mut mgr, &conn);
        let after = rendered(&mut mgr);
        assert!(after.contains("Voided invoice #1248"), "{after}");
        assert!(after.contains("page stays live"), "{after}");

        // Dismissed, the invoice itself still carries the notice.
        mgr.handle_key(KeyCode::Esc, &conn);
        let detail = rendered(&mut mgr);
        assert!(detail.contains("already published"), "{detail}");
    }

    #[test]
    fn an_unpublished_invoice_gets_no_published_warning() {
        let (_d, conn) = test_conn();
        seed_invoice(&conn, "Cedar Systems", 100.0);
        let mut mgr = manager(&conn);
        open_void(&mut mgr, &conn);

        let screen = rendered(&mut mgr);
        assert!(!screen.contains("already published"), "{screen}");
    }

    #[test]
    fn opening_a_confirmation_clears_the_status_line_and_scrolls_to_the_question() {
        let (_d, conn) = test_conn();
        seed_invoice(&conn, "Cedar Systems", 100.0);
        let mut mgr = manager(&conn);
        mgr.handle_key(KeyCode::Enter, &conn);
        mgr.set_status("something from a moment ago".into());
        mgr.handle_key(KeyCode::Char('v'), &conn);

        assert!(matches!(mgr.screen, Screen::ConfirmVoid));
        assert_eq!(mgr.status_message, None, "the y/n footer must be visible");
        let screen = rendered(&mut mgr);
        assert!(screen.contains("y=void  n=cancel"), "{screen}");
        assert!(screen.contains("Void invoice #1248"), "{screen}");
    }

    #[test]
    fn a_confirmation_on_a_long_invoice_is_scrolled_into_view() {
        let (dir, conn) = test_conn();
        // More line items than fit, so the question is off-screen unscrolled.
        let cid = add_client(&conn, "Cedar Systems", Some("ops@cedar.test"), None, None).unwrap();
        let items: Vec<NewLineItem> = (0..40)
            .map(|i| NewLineItem {
                description: format!("Line item {i}"),
                quantity: 1.0,
                unit_amount: 10.0,
            })
            .collect();
        create_invoice(&conn, cid, "2026-07-16", None, "USD", &items, None, None).unwrap();

        let mut mgr = manager(&conn);
        mgr.handle_key(KeyCode::Enter, &conn);
        mgr.begin_send(&conn, full_config(), dir.path());

        let screen = rendered(&mut mgr);
        assert!(screen.contains("Send invoice #1248"), "{screen}");
        assert!(screen.contains("y=send  n=cancel"), "{screen}");
    }

    #[test]
    fn void_writes_todays_date_as_voided_at() {
        let (_d, conn) = test_conn();
        seed_invoice(&conn, "Acme Co", 100.0);
        let mut mgr = manager(&conn);
        open_void(&mut mgr, &conn);
        confirm_void(&mut mgr, &conn);

        // refresh_status derives `void` from this column, so a wrong or empty
        // date produces an invoice that will not stay void.
        assert_eq!(voided_at(&conn), Some(crate::cli::today()));
    }

    #[test]
    fn v_on_an_already_void_invoice_is_refused_before_the_dialog() {
        let (_d, conn) = test_conn();
        let id = seed_invoice(&conn, "Acme Co", 100.0);
        void_invoice(&conn, id, "2026-08-06").unwrap();
        let mut mgr = manager(&conn);
        open_void(&mut mgr, &conn);

        assert!(matches!(mgr.screen, Screen::Detail));
        assert_eq!(
            mgr.status_message.as_deref(),
            Some("Invoice #1248 is already void.")
        );
    }

    #[test]
    fn v_on_an_invoice_with_payments_is_refused_before_the_dialog() {
        let (_d, conn) = test_conn();
        let id = seed_invoice(&conn, "Cedar Systems", 2_000.0);
        record_payment(&conn, id, 1_250.0, "2026-08-01", "ach", None).unwrap();
        let mut mgr = manager(&conn);
        open_void(&mut mgr, &conn);

        assert!(matches!(mgr.screen, Screen::Detail));
        assert_eq!(
            mgr.status_message.as_deref(),
            Some("Invoice #1248 has 1250.00 in recorded payments and cannot be voided.")
        );
        assert_eq!(voided_at(&conn), None);
    }

    #[test]
    fn a_void_invoice_cannot_then_be_paid() {
        let (_d, conn) = test_conn();
        seed_invoice(&conn, "Acme Co", 100.0);
        let mut mgr = manager(&conn);
        open_void(&mut mgr, &conn);
        confirm_void(&mut mgr, &conn);

        mgr.handle_key(KeyCode::Char('p'), &conn);
        assert!(
            matches!(mgr.screen, Screen::Detail),
            "no payment form opened"
        );
        assert_eq!(
            mgr.status_message.as_deref(),
            Some("Invoice #1248 is void and cannot be paid.")
        );
        assert_eq!(payment_rows(&conn), 0);
    }

    fn no_config() -> InvoicingConfig {
        InvoicingConfig {
            stripe_secret_key: None,
            mailgun_api_key: None,
            mailgun_domain: None,
            from_email: None,
            from_name: None,
            reply_to_email: None,
            contact_email: None,
            r2_account_id: None,
            r2_access_key: None,
            r2_secret_key: None,
            r2_bucket: None,
            public_base_url: None,
        }
    }

    fn full_config() -> InvoicingConfig {
        InvoicingConfig {
            stripe_secret_key: Some("sk_test".into()),
            mailgun_api_key: Some("key".into()),
            mailgun_domain: Some("mail.example.test".into()),
            from_email: Some("billing@mail.example.test".into()),
            from_name: None,
            reply_to_email: None,
            contact_email: None,
            r2_account_id: Some("acct".into()),
            r2_access_key: Some("ak".into()),
            r2_secret_key: Some("sk".into()),
            r2_bucket: Some("billing".into()),
            public_base_url: Some("https://billing.example.test/i".into()),
        }
    }

    /// Open the detail for the only invoice and press `s`, with an injected
    /// config and data directory so no test reads the developer's settings.
    fn begin_send(
        mgr: &mut InvoiceManager,
        conn: &Connection,
        cfg: InvoicingConfig,
        data_dir: &std::path::Path,
    ) -> InvoiceAction {
        mgr.handle_key(KeyCode::Enter, conn);
        mgr.begin_send(conn, cfg, data_dir)
    }

    #[test]
    fn s_on_a_void_invoice_is_refused_before_the_dialog() {
        let (dir, conn) = test_conn();
        let id = seed_invoice(&conn, "Cedar Systems", 100.0);
        void_invoice(&conn, id, "2026-08-06").unwrap();
        let mut mgr = manager(&conn);
        begin_send(&mut mgr, &conn, full_config(), dir.path());

        assert!(matches!(mgr.screen, Screen::Detail));
        assert_eq!(
            mgr.status_message.as_deref(),
            Some("Invoice #1248 is void and cannot be sent.")
        );
    }

    #[test]
    fn s_on_a_client_with_no_email_is_refused_before_the_dialog() {
        let (dir, conn) = test_conn();
        let cid = add_client(&conn, "Acme Co", None, None, None).unwrap();
        seed_invoice_for(&conn, cid, 100.0);
        let mut mgr = manager(&conn);
        begin_send(&mut mgr, &conn, full_config(), dir.path());

        assert!(matches!(mgr.screen, Screen::Detail));
        assert_eq!(
            mgr.status_message.as_deref(),
            Some("client 'Acme Co' has no email")
        );
    }

    #[test]
    fn s_with_missing_invoicing_config_names_the_first_absent_key() {
        let (dir, conn) = test_conn();
        seed_invoice(&conn, "Cedar Systems", 100.0);
        let mut mgr = manager(&conn);
        begin_send(&mut mgr, &conn, no_config(), dir.path());

        assert!(matches!(mgr.screen, Screen::Detail));
        let message = mgr.status_message.clone().unwrap();
        assert!(message.contains("stripe_secret_key"), "got: {message}");
    }

    #[test]
    fn s_with_a_broken_template_reports_it_and_stays_on_the_detail() {
        let (dir, conn) = test_conn();
        seed_invoice(&conn, "Cedar Systems", 100.0);
        let template = dir.path().join("templates");
        std::fs::create_dir_all(&template).unwrap();
        std::fs::write(template.join("invoice.html"), "<p>no placeholders</p>").unwrap();

        let mut mgr = manager(&conn);
        begin_send(&mut mgr, &conn, full_config(), dir.path());

        assert!(matches!(mgr.screen, Screen::Detail), "no dialog opened");
        let message = mgr.status_message.clone().unwrap();
        assert!(message.contains("invoice.html"), "got: {message}");
    }

    #[test]
    fn the_confirmation_names_the_recipient_and_total() {
        let (dir, conn) = test_conn();
        seed_invoice(&conn, "Cedar Systems", 2_000.0);
        let mut mgr = manager(&conn);
        begin_send(&mut mgr, &conn, full_config(), dir.path());

        assert!(matches!(mgr.screen, Screen::ConfirmSend));
        let screen = rendered(&mut mgr);
        assert!(
            screen.contains("Send invoice #1248 to ops@cedar.test?"),
            "{screen}"
        );
        assert!(screen.contains("Cedar Systems"), "{screen}");
        assert!(screen.contains("$2,000.00"), "{screen}");
        assert!(screen.contains("y=send  n=cancel"), "{screen}");
    }

    #[test]
    fn a_published_invoice_gets_the_resend_wording() {
        let (dir, conn) = test_conn();
        let id = seed_invoice(&conn, "Cedar Systems", 2_000.0);
        mark_published(&conn, id, "2026-07-16").unwrap();
        let mut mgr = manager(&conn);
        begin_send(&mut mgr, &conn, full_config(), dir.path());

        let screen = rendered(&mut mgr);
        assert!(screen.contains("Re-send invoice #1248"), "{screen}");
        assert!(screen.contains("Published 2026-07-16"), "{screen}");
        assert!(screen.contains("payment link is reused"), "{screen}");
    }

    #[test]
    fn n_and_esc_cancel_the_send() {
        for key in [KeyCode::Char('n'), KeyCode::Esc] {
            let (dir, conn) = test_conn();
            seed_invoice(&conn, "Cedar Systems", 100.0);
            let mut mgr = manager(&conn);
            begin_send(&mut mgr, &conn, full_config(), dir.path());
            mgr.handle_key(key, &conn);

            assert!(matches!(mgr.screen, Screen::Detail), "{key:?}");
            assert!(
                get_invoice(&conn, 1).unwrap().published_at.is_none(),
                "{key:?}"
            );
        }
    }

    #[test]
    fn y_moves_to_sending_and_returns_perform() {
        let (dir, conn) = test_conn();
        seed_invoice(&conn, "Cedar Systems", 100.0);
        let mut mgr = manager(&conn);
        begin_send(&mut mgr, &conn, full_config(), dir.path());

        let action = mgr.handle_key(KeyCode::Char('y'), &conn);
        assert!(matches!(action, InvoiceAction::Perform));
        assert!(matches!(mgr.screen, Screen::Sending));
    }

    #[test]
    fn the_sending_frame_says_the_terminal_is_frozen() {
        let (dir, conn) = test_conn();
        seed_invoice(&conn, "Cedar Systems", 100.0);
        let mut mgr = manager(&conn);
        begin_send(&mut mgr, &conn, full_config(), dir.path());
        mgr.handle_key(KeyCode::Char('y'), &conn);

        let screen = rendered(&mut mgr);
        assert!(screen.contains("Sending invoice #1248"), "{screen}");
        assert!(screen.contains("ops@cedar.test"), "{screen}");
        assert!(screen.contains("not reading keys"), "{screen}");
        assert!(screen.contains("Working"), "{screen}");
    }

    #[test]
    fn a_send_that_cannot_be_prepared_reports_it_as_a_failed_send() {
        let (dir, conn) = test_conn();
        seed_invoice(&conn, "Cedar Systems", 100.0);
        let mut mgr = manager(&conn);
        mgr.handle_key(KeyCode::Enter, &conn);
        // No config at all: nothing is built, nothing is sent.
        mgr.perform_pending_with(&conn, "2026-08-07", no_config(), dir.path());

        match &mgr.screen {
            Screen::ActionResult {
                title,
                lines,
                is_error,
            } => {
                assert_eq!(title, "Send failed");
                assert!(lines[0].contains("stripe_secret_key"), "{lines:?}");
                assert!(lines[1].contains("is still draft"), "{lines:?}");
                assert!(is_error);
            }
            _ => panic!("expected a result screen"),
        }
        assert_eq!(get_invoice(&conn, 1).unwrap().status, "draft");
    }

    /// Void with clients injected: no PDF is rendered and no template is
    /// loaded, so unlike send this runs in every build.
    mod void_teardown {
        use super::*;
        use crate::error::NigelError;
        use crate::invoicing::gateway::{PaidSession, PaymentLink};
        use crate::invoicing::invoices::set_payment_link;
        use std::cell::RefCell;

        #[derive(Default)]
        struct FakeGw {
            deactivated: RefCell<Vec<String>>,
        }
        impl PaymentGateway for FakeGw {
            fn create_payment_link(&self, _i: &Invoice, _c: &Client) -> Result<PaymentLink> {
                unreachable!("void creates nothing")
            }
            fn paid_sessions(&self, _id: &str) -> Result<Vec<PaidSession>> {
                Ok(vec![])
            }
            fn deactivate_payment_link(&self, id: &str) -> Result<()> {
                self.deactivated.borrow_mut().push(id.to_string());
                Ok(())
            }
        }

        struct FailGw;
        impl PaymentGateway for FailGw {
            fn create_payment_link(&self, _i: &Invoice, _c: &Client) -> Result<PaymentLink> {
                unreachable!("void creates nothing")
            }
            fn paid_sessions(&self, _id: &str) -> Result<Vec<PaidSession>> {
                Ok(vec![])
            }
            fn deactivate_payment_link(&self, _id: &str) -> Result<()> {
                Err(NigelError::Other(
                    "stripe 401: Invalid API Key provided".into(),
                ))
            }
        }

        #[derive(Default)]
        struct FakePub {
            pages: RefCell<u32>,
        }
        impl AssetPublisher for FakePub {
            fn publish(&self, _t: &str, _h: &[u8], _p: &[u8]) -> Result<String> {
                unreachable!("void publishes no PDF")
            }
            fn publish_page(&self, token: &str, _h: &[u8]) -> Result<String> {
                *self.pages.borrow_mut() += 1;
                Ok(format!("https://billing.example.test/i/{token}/index.html"))
            }
        }

        /// A sent invoice, as far as the teardown can tell: a link and a page.
        fn seed_sent(conn: &Connection) -> i64 {
            let id = seed_invoice(conn, "Cedar Systems", 100.0);
            set_payment_link(conn, id, "plink_1", "https://buy.stripe.com/x").unwrap();
            mark_published(conn, id, "2026-07-16").unwrap();
            id
        }

        fn open_and_confirm(mgr: &mut InvoiceManager, conn: &Connection) {
            open_void(mgr, conn);
            assert!(matches!(
                mgr.handle_key(KeyCode::Char('y'), conn),
                InvoiceAction::Perform
            ));
        }

        #[test]
        fn a_configured_void_deactivates_the_link_and_says_nothing_more() {
            let (_d, conn) = test_conn();
            seed_sent(&conn);
            let mut mgr = manager(&conn);
            open_and_confirm(&mut mgr, &conn);

            let gateway = FakeGw::default();
            let publisher = FakePub::default();
            mgr.perform_void(&conn, "2026-08-07", Some(&gateway), Some(&publisher));

            assert_eq!(*gateway.deactivated.borrow(), vec!["plink_1".to_string()]);
            assert_eq!(*publisher.pages.borrow(), 1);
            assert!(matches!(mgr.screen, Screen::Detail));
            assert_eq!(mgr.status_message.as_deref(), Some("Voided invoice #1248."));
            assert_eq!(detail_of(&mgr).invoice.status, "void");
        }

        /// AC #2 in the TUI: the invoice is voided either way, and the URL an
        /// operator has to open is on screen rather than truncated into the
        /// status line.
        #[test]
        fn a_stripe_failure_still_voids_and_shows_the_link_to_kill_by_hand() {
            let (_d, conn) = test_conn();
            seed_sent(&conn);
            let mut mgr = manager(&conn);
            open_and_confirm(&mut mgr, &conn);

            let publisher = FakePub::default();
            mgr.perform_void(&conn, "2026-08-07", Some(&FailGw), Some(&publisher));

            assert_eq!(voided_at(&conn), Some("2026-08-07".to_string()));
            let screen = rendered(&mut mgr);
            assert!(screen.contains("Voided invoice #1248"), "{screen}");
            // The address survives whole: it is wrapped onto its own line
            // rather than truncated, because it is what an operator has to open.
            assert!(screen.contains("https://buy.stripe.com/x"), "{screen}");
            // Fragments short enough to survive the wrap, whatever the width.
            assert!(screen.contains("stripe 401"), "{screen}");
        }

        #[test]
        fn the_frame_painted_before_the_work_says_the_terminal_is_frozen() {
            let (_d, conn) = test_conn();
            seed_sent(&conn);
            let mut mgr = manager(&conn);
            open_and_confirm(&mut mgr, &conn);

            let screen = rendered(&mut mgr);
            assert!(screen.contains("Voiding invoice #1248"), "{screen}");
            assert!(screen.contains("not reading keys"), "{screen}");
            assert!(screen.contains("Working"), "{screen}");
        }

        /// The frame names two network calls, so an invoice that is going to
        /// make neither must never see it: a draft void is a database write and
        /// runs inline, on the keypress.
        #[test]
        fn a_draft_void_paints_no_blocking_frame_and_needs_no_second_pass() {
            let (_d, conn) = test_conn();
            seed_invoice(&conn, "Cedar Systems", 100.0);
            let mut mgr = manager(&conn);
            open_void(&mut mgr, &conn);

            assert!(matches!(
                mgr.handle_key(KeyCode::Char('y'), &conn),
                InvoiceAction::Continue
            ));

            // Voided already, without the controller doing anything more.
            assert_eq!(voided_at(&conn), Some(crate::cli::today()));
            assert!(matches!(mgr.screen, Screen::Detail));
            assert_eq!(mgr.status_message.as_deref(), Some("Voided invoice #1248."));
            let screen = rendered(&mut mgr);
            assert!(!screen.contains("Voiding invoice"), "{screen}");
            assert!(!screen.contains("Stripe payment link"), "{screen}");
        }

        /// Half a reason is reason enough: an unpublished draft that carries a
        /// link has a call to make, so it takes the long way round.
        #[test]
        fn an_unpublished_invoice_with_a_link_still_gets_the_frame() {
            let (_d, conn) = test_conn();
            let id = seed_invoice(&conn, "Cedar Systems", 100.0);
            set_payment_link(&conn, id, "plink_1", "https://buy.stripe.com/x").unwrap();
            let mut mgr = manager(&conn);
            open_void(&mut mgr, &conn);

            assert!(matches!(
                mgr.handle_key(KeyCode::Char('y'), &conn),
                InvoiceAction::Perform
            ));
            assert!(matches!(mgr.screen, Screen::Voiding));
            assert_eq!(
                voided_at(&conn),
                None,
                "nothing is written until the work runs"
            );
        }
    }

    // Sending needs a real PDF to publish and attach, so the orchestration is
    // only exercisable in a `pdf` build — the same gate invoicing::send uses.
    #[cfg(feature = "pdf")]
    mod send {
        use super::*;
        use crate::error::NigelError;
        use crate::invoicing::gateway::{PaidSession, PaymentLink};
        use crate::invoicing::render_html::DEFAULT_TEMPLATE;
        use std::cell::RefCell;

        struct FakeGw {
            create_calls: RefCell<u32>,
        }
        impl PaymentGateway for FakeGw {
            fn deactivate_payment_link(&self, _id: &str) -> Result<()> {
                unreachable!("deactivation belongs to void, not to this path")
            }
            fn create_payment_link(&self, _i: &Invoice, _c: &Client) -> Result<PaymentLink> {
                *self.create_calls.borrow_mut() += 1;
                Ok(PaymentLink {
                    id: "pl_1".into(),
                    url: "https://pay/x".into(),
                })
            }
            fn paid_sessions(&self, _id: &str) -> Result<Vec<PaidSession>> {
                Ok(vec![])
            }
        }
        struct FakePub;
        impl AssetPublisher for FakePub {
            fn publish(&self, token: &str, _h: &[u8], _p: &[u8]) -> Result<String> {
                Ok(format!("https://billing.example.com/i/{token}/index.html"))
            }
            fn publish_page(&self, token: &str, _h: &[u8]) -> Result<String> {
                Ok(format!("https://billing.example.com/i/{token}/index.html"))
            }
        }
        struct FailPub;
        impl AssetPublisher for FailPub {
            fn publish(&self, _t: &str, _h: &[u8], _p: &[u8]) -> Result<String> {
                Err(NigelError::Other("upload down".into()))
            }
            fn publish_page(&self, _t: &str, _h: &[u8]) -> Result<String> {
                Err(NigelError::Other("upload down".into()))
            }
        }
        #[derive(Default)]
        struct FakeMail {
            sent: RefCell<u32>,
        }
        impl Mailer for FakeMail {
            fn send_invoice(&self, _to: &str, _s: &str, _h: &str, _p: &[u8]) -> Result<()> {
                *self.sent.borrow_mut() += 1;
                Ok(())
            }
        }

        fn gateway() -> FakeGw {
            FakeGw {
                create_calls: RefCell::new(0),
            }
        }

        fn branding() -> Branding<'static> {
            Branding {
                template: DEFAULT_TEMPLATE,
                company: "",
                contact_email: "billing@example.test",
            }
        }

        fn result_of(mgr: &InvoiceManager) -> (&str, &[String], bool) {
            match &mgr.screen {
                Screen::ActionResult {
                    title,
                    lines,
                    is_error,
                } => (title.as_str(), lines.as_slice(), *is_error),
                _ => panic!("expected a result screen"),
            }
        }

        #[test]
        fn a_successful_send_publishes_emails_and_shows_the_url() {
            let (_d, conn) = test_conn();
            let id = seed_invoice(&conn, "Cedar Systems", 100.0);
            let mut mgr = manager(&conn);
            mgr.handle_key(KeyCode::Enter, &conn);
            let mail = FakeMail::default();
            mgr.perform_send(
                &conn,
                "2026-08-07",
                &branding(),
                &gateway(),
                &FakePub,
                &mail,
            );

            let (title, lines, is_error) = result_of(&mgr);
            assert_eq!(title, "Invoice #1248 sent");
            assert!(
                lines[0].starts_with("https://billing.example.com/i/"),
                "{lines:?}"
            );
            assert_eq!(lines[1], "Emailed to ops@cedar.test.");
            assert!(!is_error);

            assert_eq!(get_invoice(&conn, id).unwrap().status, "sent");
            assert_eq!(
                detail_of(&mgr).invoice.status,
                "sent",
                "the detail reloaded"
            );
            assert_eq!(mgr.rows[0].status, "sent", "the list reloaded");
            assert_eq!(*mail.sent.borrow(), 1);
        }

        #[test]
        fn a_failed_send_reports_the_error_verbatim_and_the_reloaded_status() {
            let (_d, conn) = test_conn();
            let id = seed_invoice(&conn, "Cedar Systems", 100.0);
            let mut mgr = manager(&conn);
            mgr.handle_key(KeyCode::Enter, &conn);
            let mail = FakeMail::default();
            mgr.perform_send(
                &conn,
                "2026-08-07",
                &branding(),
                &gateway(),
                &FailPub,
                &mail,
            );

            let (title, lines, is_error) = result_of(&mgr);
            assert_eq!(title, "Send failed");
            assert_eq!(lines[0], "upload down");
            assert_eq!(
                lines[1],
                "Invoice #1248 is still draft. Nothing was published or emailed."
            );
            assert!(is_error);
            assert_eq!(get_invoice(&conn, id).unwrap().status, "draft");
            assert_eq!(*mail.sent.borrow(), 0);
        }

        #[test]
        fn a_failed_resend_says_the_invoice_is_still_sent_not_still_draft() {
            let (_d, conn) = test_conn();
            seed_invoice(&conn, "Cedar Systems", 100.0);
            let mut mgr = manager(&conn);
            mgr.handle_key(KeyCode::Enter, &conn);
            let mail = FakeMail::default();
            mgr.perform_send(
                &conn,
                "2026-08-07",
                &branding(),
                &gateway(),
                &FakePub,
                &mail,
            );
            mgr.handle_key(KeyCode::Esc, &conn); // dismiss the result
            mgr.perform_send(
                &conn,
                "2026-08-08",
                &branding(),
                &gateway(),
                &FailPub,
                &mail,
            );

            let (_, lines, _) = result_of(&mgr);
            assert_eq!(
                lines[1],
                "Invoice #1248 is still sent. Nothing was published or emailed."
            );
        }

        #[test]
        fn a_resend_reuses_the_existing_payment_link() {
            let (_d, conn) = test_conn();
            seed_invoice(&conn, "Cedar Systems", 100.0);
            let mut mgr = manager(&conn);
            mgr.handle_key(KeyCode::Enter, &conn);
            let mail = FakeMail::default();
            let gw = gateway();
            mgr.perform_send(&conn, "2026-08-07", &branding(), &gw, &FakePub, &mail);
            mgr.handle_key(KeyCode::Esc, &conn);
            mgr.perform_send(&conn, "2026-08-08", &branding(), &gw, &FakePub, &mail);

            assert_eq!(*gw.create_calls.borrow(), 1);
            assert_eq!(*mail.sent.borrow(), 2);
        }

        #[test]
        fn any_key_on_the_result_returns_to_the_reloaded_detail() {
            let (_d, conn) = test_conn();
            seed_invoice(&conn, "Cedar Systems", 100.0);
            let mut mgr = manager(&conn);
            mgr.handle_key(KeyCode::Enter, &conn);
            mgr.perform_send(
                &conn,
                "2026-08-07",
                &branding(),
                &gateway(),
                &FakePub,
                &FakeMail::default(),
            );

            mgr.handle_key(KeyCode::Enter, &conn);
            assert!(matches!(mgr.screen, Screen::Detail));
            assert_eq!(detail_of(&mgr).invoice.status, "sent");
        }

        #[test]
        fn the_result_screen_renders_its_lines() {
            let (_d, conn) = test_conn();
            seed_invoice(&conn, "Cedar Systems", 100.0);
            let mut mgr = manager(&conn);
            mgr.handle_key(KeyCode::Enter, &conn);
            mgr.perform_send(
                &conn,
                "2026-08-07",
                &branding(),
                &gateway(),
                &FakePub,
                &FakeMail::default(),
            );

            let screen = rendered(&mut mgr);
            assert!(screen.contains("Invoice #1248 sent"), "{screen}");
            assert!(screen.contains("Emailed to ops@cedar.test."), "{screen}");
            assert!(screen.contains("Esc=back"), "{screen}");
        }
    }

    /// The screen as an 80x24 terminal renders it, one string per row.
    fn rendered(mgr: &mut InvoiceManager) -> String {
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
    fn the_list_renders_its_columns_inside_eighty_columns() {
        let (_d, conn) = test_conn();
        let id = seed_invoice(&conn, "Cedar Systems", 2_000.0);
        record_payment(&conn, id, 1_250.0, "2026-08-01", "ach", None).unwrap();
        let mut mgr = manager(&conn);

        let screen = rendered(&mut mgr);
        assert!(screen.contains("Invoices (1)"), "{screen}");
        for column in ["#", "Status", "Client", "Total", "Balance", "Due"] {
            assert!(screen.contains(column), "{column} missing:\n{screen}");
        }
        assert!(screen.contains("> 1248"), "{screen}");
        assert!(screen.contains("$2,000.00"), "{screen}");
        assert!(screen.contains("$750.00"), "{screen}");
        assert!(screen.contains("2026-08-15"), "{screen}");
        assert!(screen.contains("Enter=open  Esc=back  q=quit"), "{screen}");
    }

    fn row_of(conn: &Connection) -> InvoiceListRow {
        list_invoices(conn, None, None).unwrap().pop().unwrap()
    }

    #[test]
    fn a_row_fits_eighty_columns_even_at_seven_figures() {
        // A rendered frame is 80 cells wide by construction, so a row that
        // overruns is invisible in a TestBackend buffer: the budget is checked
        // on the string, not on what survived being drawn.
        let (_d, conn) = test_conn();
        let cid = add_client(
            &conn,
            &"Wintermute Consolidated Ltd".repeat(2),
            None,
            None,
            None,
        )
        .unwrap();
        seed_invoice_for(&conn, cid, 9_999_999.99);
        let row = row_of(&conn);

        let rendered = list_row(&row);
        assert!(
            rendered.chars().count() <= ROW_WIDTH,
            "row is {} cols: {rendered:?}",
            rendered.chars().count()
        );
        // The client name yields; the due date is not pushed off the end.
        assert!(rendered.ends_with("2026-08-15"), "{rendered:?}");
        assert!(rendered.contains("$9,999,999.99"), "{rendered:?}");
    }

    #[test]
    fn an_ordinary_row_keeps_the_specced_columns() {
        let (_d, conn) = test_conn();
        let id = seed_invoice(&conn, "Cedar Systems", 2_000.0);
        record_payment(&conn, id, 1_250.0, "2026-08-01", "ach", None).unwrap();

        let rendered = list_row(&row_of(&conn));
        assert_eq!(rendered.chars().count(), ROW_WIDTH);
        assert!(rendered.starts_with("   1248   "), "{rendered:?}");
        assert!(rendered.contains("Cedar Systems"), "{rendered:?}");
        assert!(rendered.contains("$2,000.00"), "{rendered:?}");
        assert!(rendered.contains("$750.00"), "{rendered:?}");
    }

    #[test]
    fn every_screen_survives_a_terminal_narrower_than_its_columns() {
        let (dir, conn) = test_conn();
        let id = seed_invoice(&conn, "Cedar Systems", 2_000.0);
        record_payment(&conn, id, 1_250.0, "2026-08-01", "ach", None).unwrap();
        let mut mgr = manager(&conn);

        let mut narrow =
            ratatui::Terminal::new(ratatui::backend::TestBackend::new(40, 10)).unwrap();
        let mut draw = |mgr: &mut InvoiceManager| {
            narrow.draw(|frame| mgr.draw(frame)).unwrap();
        };

        draw(&mut mgr); // list
        mgr.handle_key(KeyCode::Enter, &conn);
        draw(&mut mgr); // detail
        mgr.handle_key(KeyCode::Char('v'), &conn);
        draw(&mut mgr); // confirm void
        mgr.handle_key(KeyCode::Char('n'), &conn);
        mgr.handle_key(KeyCode::Char('p'), &conn);
        draw(&mut mgr); // payment form
        mgr.handle_key(KeyCode::Esc, &conn);
        mgr.begin_send(&conn, full_config(), dir.path());
        draw(&mut mgr); // confirm send
        mgr.handle_key(KeyCode::Char('y'), &conn);
        draw(&mut mgr); // sending

        let mut mgr = manager(&conn);
        open_form(&mut mgr, &conn);
        fill_row(&mut mgr, &conn, 0, ("Strategy workshop", "2", "1500"));
        draw(&mut mgr); // draft form
    }

    #[test]
    fn the_empty_list_points_at_the_form_not_at_the_cli() {
        let (_d, conn) = test_conn();
        let mut mgr = manager(&conn);
        let screen = rendered(&mut mgr);
        assert!(screen.contains("Invoices (0)"), "{screen}");
        assert!(screen.contains("Press 'a' to draft one"), "{screen}");
        assert!(!screen.contains("nigel invoice new"), "{screen}");
    }

    fn new_form(mgr: &InvoiceManager) -> &InvoiceForm {
        match &mgr.screen {
            Screen::NewInvoice(form) => form,
            _ => panic!("not on the draft form"),
        }
    }

    fn open_form(mgr: &mut InvoiceManager, conn: &Connection) {
        mgr.handle_key(KeyCode::Char('a'), conn);
    }

    fn invoice_rows(conn: &Connection) -> i64 {
        conn.query_row("SELECT COUNT(*) FROM invoices", [], |r| r.get(0))
            .unwrap()
    }

    /// Two clients and no invoices — the state the form is opened from.
    fn seed_clients(conn: &Connection) {
        add_client(conn, "Cedar Systems", Some("ops@cedar.test"), None, None).unwrap();
        add_client(conn, "Harbor & Vale", Some("ap@harbor.test"), None, None).unwrap();
    }

    #[test]
    fn a_opens_the_draft_form_prefilled() {
        let (_d, conn) = test_conn();
        seed_clients(&conn);
        let mut mgr = manager(&conn);
        open_form(&mut mgr, &conn);

        let form = new_form(&mgr);
        assert_eq!(form.client().name, "Cedar Systems");
        assert_eq!(form.issue_date, crate::cli::today());
        assert_eq!(form.due_date, plus_days(&crate::cli::today(), 30).unwrap());
        assert_eq!(form.currency, "USD");
        assert_eq!(form.items.len(), 1);
        assert_eq!(form.focused, CLIENT_IDX);
    }

    #[test]
    fn n_opens_the_same_form_as_a() {
        let (_d, conn) = test_conn();
        seed_clients(&conn);
        let mut mgr = manager(&conn);
        mgr.handle_key(KeyCode::Char('n'), &conn);
        assert!(matches!(mgr.screen, Screen::NewInvoice(_)));
    }

    #[test]
    fn the_form_opens_from_an_empty_list_too() {
        let (_d, conn) = test_conn();
        seed_clients(&conn);
        let mut mgr = manager(&conn);
        assert!(mgr.rows.is_empty());
        open_form(&mut mgr, &conn);
        assert!(matches!(mgr.screen, Screen::NewInvoice(_)));
    }

    #[test]
    fn a_with_no_clients_is_refused_before_the_form_opens() {
        let (_d, conn) = test_conn();
        let mut mgr = manager(&conn);
        open_form(&mut mgr, &conn);

        assert!(matches!(mgr.screen, Screen::List));
        assert_eq!(
            mgr.status_message.as_deref(),
            Some("No clients yet. Add one on the Clients screen first.")
        );
    }

    #[test]
    fn left_and_right_cycle_the_client() {
        let (_d, conn) = test_conn();
        seed_clients(&conn);
        let mut mgr = manager(&conn);
        open_form(&mut mgr, &conn);

        mgr.handle_key(KeyCode::Right, &conn);
        assert_eq!(new_form(&mgr).client().name, "Harbor & Vale");
        mgr.handle_key(KeyCode::Right, &conn);
        assert_eq!(new_form(&mgr).client().name, "Cedar Systems", "it wraps");
        mgr.handle_key(KeyCode::Left, &conn);
        assert_eq!(new_form(&mgr).client().name, "Harbor & Vale");
    }

    /// Tab around to a given field index.
    fn focus_field(mgr: &mut InvoiceManager, conn: &Connection, idx: usize) {
        while new_form(mgr).focused != idx {
            mgr.handle_key(KeyCode::Tab, conn);
        }
    }

    #[test]
    fn tab_walks_every_header_field_and_every_cell_then_wraps() {
        let (_d, conn) = test_conn();
        seed_clients(&conn);
        let mut mgr = manager(&conn);
        open_form(&mut mgr, &conn);
        let count = new_form(&mgr).field_count();
        assert_eq!(
            count,
            HEADER_FIELDS + CELLS_PER_ROW,
            "one row of three cells"
        );

        for expected in 1..count {
            mgr.handle_key(KeyCode::Tab, &conn);
            assert_eq!(new_form(&mgr).focused, expected);
        }
        mgr.handle_key(KeyCode::Tab, &conn);
        assert_eq!(new_form(&mgr).focused, 0, "Tab wraps");
        mgr.handle_key(KeyCode::BackTab, &conn);
        assert_eq!(new_form(&mgr).focused, count - 1, "and so does BackTab");
    }

    #[test]
    fn insert_adds_a_line_below_the_focused_row_and_delete_removes_it() {
        let (_d, conn) = test_conn();
        seed_clients(&conn);
        let mut mgr = manager(&conn);
        open_form(&mut mgr, &conn);
        focus_field(&mut mgr, &conn, InvoiceForm::cell_index(0, DESC_CELL));
        type_str(&mut mgr, &conn, "First");

        mgr.handle_key(KeyCode::Insert, &conn);
        assert_eq!(new_form(&mgr).items.len(), 2);
        assert_eq!(
            new_form(&mgr).focused,
            InvoiceForm::cell_index(1, DESC_CELL),
            "the new row takes focus"
        );
        type_str(&mut mgr, &conn, "Second");
        assert_eq!(new_form(&mgr).items[0].description, "First");
        assert_eq!(new_form(&mgr).items[1].description, "Second");

        mgr.handle_key(KeyCode::Delete, &conn);
        assert_eq!(new_form(&mgr).items.len(), 1);
        assert_eq!(new_form(&mgr).items[0].description, "First");
    }

    #[test]
    fn f2_and_f3_are_bound_alongside_insert_and_delete() {
        let (_d, conn) = test_conn();
        seed_clients(&conn);
        let mut mgr = manager(&conn);
        open_form(&mut mgr, &conn);
        focus_field(&mut mgr, &conn, InvoiceForm::cell_index(0, DESC_CELL));

        mgr.handle_key(KeyCode::F(2), &conn);
        assert_eq!(new_form(&mgr).items.len(), 2);
        mgr.handle_key(KeyCode::F(3), &conn);
        assert_eq!(new_form(&mgr).items.len(), 1);
    }

    #[test]
    fn the_last_line_cannot_be_removed_and_says_so_in_the_data_layers_words() {
        let (_d, conn) = test_conn();
        seed_clients(&conn);
        let mut mgr = manager(&conn);
        open_form(&mut mgr, &conn);
        focus_field(&mut mgr, &conn, InvoiceForm::cell_index(0, DESC_CELL));
        mgr.handle_key(KeyCode::Delete, &conn);

        assert_eq!(new_form(&mgr).items.len(), 1);
        let (_, message) = new_form(&mgr).error.clone().unwrap();
        assert_eq!(message, "An invoice needs at least one line item.");
    }

    #[test]
    fn delete_on_a_header_field_removes_nothing() {
        let (_d, conn) = test_conn();
        seed_clients(&conn);
        let mut mgr = manager(&conn);
        open_form(&mut mgr, &conn);
        mgr.handle_key(KeyCode::Insert, &conn);
        focus_field(&mut mgr, &conn, ISSUE_IDX);
        mgr.handle_key(KeyCode::Delete, &conn);
        assert_eq!(new_form(&mgr).items.len(), 2);
    }

    /// Type one line item into an existing row.
    fn fill_row(mgr: &mut InvoiceManager, conn: &Connection, row: usize, item: (&str, &str, &str)) {
        focus_field(mgr, conn, InvoiceForm::cell_index(row, DESC_CELL));
        type_str(mgr, conn, item.0);
        mgr.handle_key(KeyCode::Tab, conn);
        type_str(mgr, conn, item.1);
        mgr.handle_key(KeyCode::Tab, conn);
        type_str(mgr, conn, item.2);
    }

    #[test]
    fn a_completed_form_creates_a_draft_with_every_line_item() {
        let (_d, conn) = test_conn();
        seed_clients(&conn);
        let mut mgr = manager(&conn);
        open_form(&mut mgr, &conn);
        mgr.handle_key(KeyCode::Right, &conn); // Harbor & Vale
        fill_row(&mut mgr, &conn, 0, ("Strategy workshop", "2", "1500"));
        mgr.handle_key(KeyCode::Insert, &conn);
        fill_row(&mut mgr, &conn, 1, ("Technical audit", "1", "2,200.50"));
        mgr.handle_key(KeyCode::Enter, &conn);

        assert!(matches!(mgr.screen, Screen::List), "back to the list");
        assert_eq!(invoice_rows(&conn), 1);

        let invoice = get_invoice(&conn, 1).unwrap();
        assert_eq!(invoice.status, "draft");
        assert_eq!(invoice.currency, "USD");
        assert_eq!(invoice.issue_date, crate::cli::today());
        assert_eq!(
            invoice.due_date.as_deref(),
            Some(plus_days(&crate::cli::today(), 30).unwrap().as_str())
        );
        assert_eq!(invoice.total, 5_200.50);
        assert!(invoice.published_at.is_none());
        assert!(invoice.stripe_payment_link_id.is_none());

        let items = line_items(&conn, invoice.id).unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].description, "Strategy workshop");
        assert_eq!(items[0].quantity, 2.0);
        assert_eq!(items[0].line_total, 3_000.0);
        assert_eq!(items[1].description, "Technical audit");
        assert_eq!(items[1].line_total, 2_200.50);

        let client = get_client(&conn, invoice.client_id).unwrap();
        assert_eq!(client.name, "Harbor & Vale");

        assert_eq!(mgr.rows.len(), 1, "the list reloaded");
        assert_eq!(mgr.selection, 0, "on the new invoice");
        assert_eq!(
            mgr.status_message.as_deref(),
            Some("Created draft invoice #1248 for $5,200.50.")
        );
    }

    #[test]
    fn a_blank_due_date_creates_an_invoice_that_never_goes_overdue() {
        let (_d, conn) = test_conn();
        seed_clients(&conn);
        let mut mgr = manager(&conn);
        open_form(&mut mgr, &conn);
        focus_field(&mut mgr, &conn, DUE_IDX);
        clear_field(&mut mgr, &conn);
        fill_row(&mut mgr, &conn, 0, ("Retainer", "1", "1000"));
        mgr.handle_key(KeyCode::Enter, &conn);

        assert_eq!(get_invoice(&conn, 1).unwrap().due_date, None);
    }

    #[test]
    fn the_currency_field_uppercases_what_is_typed() {
        let (_d, conn) = test_conn();
        seed_clients(&conn);
        let mut mgr = manager(&conn);
        open_form(&mut mgr, &conn);
        focus_field(&mut mgr, &conn, CURRENCY_IDX);
        clear_field(&mut mgr, &conn);
        type_str(&mut mgr, &conn, "eur1-");
        assert_eq!(new_form(&mgr).currency, "EUR");
    }

    #[test]
    fn every_refusal_is_the_data_layers_own_and_writes_nothing() {
        // (issue, due, currency, quantity, unit, message, the field it belongs to)
        let cases: [(&str, &str, &str, &str, &str, &str, usize); 6] = [
            (
                "2026-13-45",
                "",
                "USD",
                "1",
                "100",
                "Invalid issue date: 2026-13-45 (expected YYYY-MM-DD)",
                ISSUE_IDX,
            ),
            (
                "2026-08-10",
                "2026-13-45",
                "USD",
                "1",
                "100",
                "Invalid due date: 2026-13-45 (expected YYYY-MM-DD)",
                DUE_IDX,
            ),
            (
                "2026-08-10",
                "",
                "US",
                "1",
                "100",
                "Invalid currency: US (expected a 3-letter code like USD)",
                CURRENCY_IDX,
            ),
            (
                "2026-08-10",
                "",
                "USD",
                "0",
                "100",
                "An invoice must total more than zero, got 0.00.",
                InvoiceForm::cell_index(0, DESC_CELL),
            ),
            (
                "2026-08-10",
                "",
                "USD",
                "",
                "100",
                "Quantity is required",
                InvoiceForm::cell_index(0, QTY_CELL),
            ),
            (
                "2026-08-10",
                "",
                "USD",
                "1",
                "",
                "Unit amount is required",
                InvoiceForm::cell_index(0, UNIT_CELL),
            ),
        ];

        for (issue, due, currency, quantity, unit, expected, anchor) in cases {
            let (_d, conn) = test_conn();
            seed_clients(&conn);
            let mut mgr = manager(&conn);
            open_form(&mut mgr, &conn);
            for (idx, value) in [(ISSUE_IDX, issue), (DUE_IDX, due), (CURRENCY_IDX, currency)] {
                focus_field(&mut mgr, &conn, idx);
                clear_field(&mut mgr, &conn);
                type_str(&mut mgr, &conn, value);
            }
            fill_row(&mut mgr, &conn, 0, ("Consulting", quantity, unit));
            mgr.handle_key(KeyCode::Enter, &conn);

            let (got_anchor, message) = new_form(&mgr)
                .error
                .clone()
                .unwrap_or_else(|| panic!("no refusal for {expected}"));
            assert_eq!(message, expected);
            assert_eq!(got_anchor, anchor, "{expected} is anchored to its field");
            assert_eq!(new_form(&mgr).focused, anchor, "focus moved to it");
            assert_eq!(invoice_rows(&conn), 0, "{expected} wrote a row");
        }
    }

    #[test]
    fn an_unparseable_figure_names_the_field_not_the_cli_flag() {
        let (_d, conn) = test_conn();
        seed_clients(&conn);
        let mut mgr = manager(&conn);
        open_form(&mut mgr, &conn);
        // The cells take digits, `.` and `,`, so `..` is the only junk typable.
        fill_row(&mut mgr, &conn, 0, ("Consulting", "..", "100"));
        mgr.handle_key(KeyCode::Enter, &conn);

        let (_, message) = new_form(&mgr).error.clone().unwrap();
        assert_eq!(message, "Quantity must be a number");
        assert!(!message.contains("--item"), "got: {message}");
    }

    #[test]
    fn a_refusal_renders_beside_the_field_it_is_about() {
        let (_d, conn) = test_conn();
        seed_clients(&conn);
        let mut mgr = manager(&conn);
        open_form(&mut mgr, &conn);
        focus_field(&mut mgr, &conn, DUE_IDX);
        clear_field(&mut mgr, &conn);
        type_str(&mut mgr, &conn, "2026-13-45");
        fill_row(&mut mgr, &conn, 0, ("Consulting", "1", "100"));
        mgr.handle_key(KeyCode::Enter, &conn);

        let screen = rendered(&mut mgr);
        let lines: Vec<&str> = screen.lines().collect();
        let due = lines
            .iter()
            .position(|l| l.contains("Due date"))
            .expect("no due date field");
        assert!(
            lines[due + 1].contains("Invalid due date: 2026-13-45 (expected YYYY-MM-DD)"),
            "the message is not under the field:\n{screen}"
        );
    }

    #[test]
    fn esc_cancels_the_draft_without_writing() {
        let (_d, conn) = test_conn();
        seed_clients(&conn);
        let mut mgr = manager(&conn);
        open_form(&mut mgr, &conn);
        fill_row(&mut mgr, &conn, 0, ("Consulting", "1", "100"));
        mgr.handle_key(KeyCode::Esc, &conn);

        assert!(matches!(mgr.screen, Screen::List));
        assert_eq!(invoice_rows(&conn), 0);
    }

    #[test]
    fn q_types_into_the_form_instead_of_quitting() {
        let (_d, conn) = test_conn();
        seed_clients(&conn);
        let mut mgr = manager(&conn);
        open_form(&mut mgr, &conn);
        focus_field(&mut mgr, &conn, InvoiceForm::cell_index(0, DESC_CELL));
        let action = mgr.handle_key(KeyCode::Char('q'), &conn);

        assert!(!is_close(action));
        assert_eq!(new_form(&mgr).items[0].description, "q");
    }

    #[test]
    fn the_form_renders_its_fields_the_running_totals_and_its_keys() {
        let (_d, conn) = test_conn();
        seed_clients(&conn);
        let mut mgr = manager(&conn);
        open_form(&mut mgr, &conn);
        fill_row(&mut mgr, &conn, 0, ("Strategy workshop", "2", "1500"));
        mgr.handle_key(KeyCode::Insert, &conn);
        fill_row(&mut mgr, &conn, 1, ("Technical audit", "1", "200"));

        let screen = rendered(&mut mgr);
        assert!(screen.contains("New Draft Invoice"), "{screen}");
        assert!(screen.contains("Cedar Systems"), "{screen}");
        for label in ["Client", "Issue date", "Due date", "Currency"] {
            assert!(screen.contains(label), "{label} missing:\n{screen}");
        }
        assert!(screen.contains("Strategy workshop"), "{screen}");
        assert!(screen.contains("3000.00"), "the line amount:\n{screen}");
        assert!(screen.contains("$3,200.00"), "the running total:\n{screen}");
        assert!(
            screen.contains("Ins/F2=add line  Del/F3=remove line  Enter=create  Esc=cancel"),
            "{screen}"
        );
    }

    #[test]
    fn an_incomplete_row_shows_an_em_dash_rather_than_a_zero_amount() {
        let (_d, conn) = test_conn();
        seed_clients(&conn);
        let mut mgr = manager(&conn);
        open_form(&mut mgr, &conn);
        focus_field(&mut mgr, &conn, InvoiceForm::cell_index(0, DESC_CELL));
        type_str(&mut mgr, &conn, "Not priced yet");

        assert_eq!(new_form(&mgr).items[0].amount(), None);
        assert_eq!(new_form(&mgr).total(), 0.0);
        let screen = rendered(&mut mgr);
        assert!(screen.contains('\u{2014}'), "{screen}");
    }

    /// A form taller than the terminal scrolls to follow the focus, and the
    /// refusal renders *below* the field it is about — so a window that stops
    /// at the focused line clips the very sentence that explains why Enter
    /// appeared to do nothing.
    #[test]
    fn a_refusal_on_a_late_line_item_is_scrolled_into_view() {
        let (_d, conn) = test_conn();
        seed_clients(&conn);
        let mut mgr = manager(&conn);
        open_form(&mut mgr, &conn);
        for row in 0..15 {
            if row > 0 {
                mgr.handle_key(KeyCode::Insert, &conn);
            }
            // Only the last row is bad, so validation reaches it.
            let quantity = if row == 14 { ".." } else { "1" };
            fill_row(&mut mgr, &conn, row, ("Consulting", quantity, "100"));
        }
        mgr.handle_key(KeyCode::Enter, &conn);

        assert_eq!(
            new_form(&mgr).error.clone().map(|(_, m)| m).unwrap(),
            "Quantity must be a number"
        );
        let screen = rendered(&mut mgr);
        assert!(
            screen.contains("Quantity must be a number"),
            "the refusal is off screen:\n{screen}"
        );
        assert_eq!(invoice_rows(&conn), 0);
    }

    #[test]
    fn a_line_item_row_fits_eighty_columns_at_eight_figures() {
        let (_d, conn) = test_conn();
        seed_clients(&conn);
        let mut mgr = manager(&conn);
        open_form(&mut mgr, &conn);
        fill_row(
            &mut mgr,
            &conn,
            0,
            (
                &"Wintermute Consolidated retainer ".repeat(3),
                "999",
                "99999999.99",
            ),
        );

        let rows = item_table_strings(new_form(&mgr));
        for row in &rows {
            assert!(
                row.chars().count() <= ROW_WIDTH,
                "row is {} cols: {row:?}",
                row.chars().count()
            );
        }
        // The description is what yields; the figures are never truncated.
        assert!(rows[1].contains("99899999990.01"), "{rows:#?}");
        assert!(rows[1].contains("99999999.99"), "{rows:#?}");
        assert!(rows[1].contains('\u{2026}'), "{rows:#?}");
        // And the header and the total keep the same columns.
        assert!(rows[0].ends_with("Amount"), "{:?}", rows[0]);
        assert_eq!(
            rows[0].chars().count(),
            rows[1].chars().count(),
            "header and row widths drifted"
        );
        assert_eq!(
            rows[0].chars().count(),
            rows[2].chars().count(),
            "header and total widths drifted"
        );
    }

    #[test]
    fn only_the_client_field_advertises_left_and_right() {
        let (_d, conn) = test_conn();
        seed_clients(&conn);
        let mut mgr = manager(&conn);
        open_form(&mut mgr, &conn);
        assert!(rendered(&mut mgr).contains("Left/Right=client"));

        focus_field(&mut mgr, &conn, ISSUE_IDX);
        let screen = rendered(&mut mgr);
        assert!(!screen.contains("Left/Right"), "{screen}");
        assert!(screen.contains("Tab=field  Ins/F2=add line"), "{screen}");

        focus_field(&mut mgr, &conn, InvoiceForm::cell_index(0, DESC_CELL));
        let screen = rendered(&mut mgr);
        assert!(!screen.contains("Left/Right"), "{screen}");
        assert!(screen.contains("Del/F3=remove line"), "{screen}");
    }

    #[test]
    fn an_unreadable_clients_table_is_reported_not_read_as_an_empty_book() {
        let (_d, conn) = test_conn();
        seed_clients(&conn);
        let mut mgr = manager(&conn);
        conn.execute("PRAGMA foreign_keys = OFF", []).unwrap();
        conn.execute("DROP TABLE clients", []).unwrap();
        open_form(&mut mgr, &conn);

        assert!(matches!(mgr.screen, Screen::List));
        let message = mgr.status_message.clone().unwrap();
        assert!(!message.starts_with("No clients yet"), "got: {message}");
        assert!(message.contains("clients"), "got: {message}");
    }

    #[test]
    fn the_list_footer_offers_the_form() {
        let (_d, conn) = test_conn();
        seed_invoice(&conn, "Cedar Systems", 100.0);
        let mut mgr = manager(&conn);
        let screen = rendered(&mut mgr);
        assert!(
            screen.contains("a=new  Enter=open  Esc=back  q=quit"),
            "{screen}"
        );
    }

    #[test]
    fn a_missing_due_date_renders_as_an_em_dash() {
        let (_d, conn) = test_conn();
        let cid = add_client(&conn, "Acme Co", None, None, None).unwrap();
        let items = vec![NewLineItem {
            description: "Retainer".into(),
            quantity: 1.0,
            unit_amount: 1_250.0,
        }];
        create_invoice(&conn, cid, "2026-08-06", None, "USD", &items, None, None).unwrap();
        let mut mgr = manager(&conn);

        assert!(rendered(&mut mgr).contains('\u{2014}'));
    }
}
